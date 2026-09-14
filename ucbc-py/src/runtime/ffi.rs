//! Thin wrappers over the parts of the CPython C API this runtime needs: a
//! subinterpreter's lifecycle, its warden, and running bootstrap code in it.
//! Everything `unsafe` in the runtime lives here and in `bridge`.
//!
//! Nothing in the runtime may use PyO3 while a bot's interpreter is attached: the
//! thread's GIL-state slot is bound to the main interpreter, so `PyGILState_Ensure`
//! would attach to the wrong one.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_long, c_ulong};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use pyo3::ffi;

/// A subinterpreter with its own GIL, owned by the calling thread. Not panic-safe:
/// a panic while attached leaves the GIL held.
pub(super) struct Interp {
    tstate: *mut ffi::PyThreadState,
}

impl Interp {
    /// Creates the interpreter. Attaches to the main interpreter only for the call
    /// and leaves the thread detached from everything. `PyGILState_Ensure` comes
    /// first so the new interpreter's thread state never becomes this thread's
    /// GIL-state binding.
    pub(super) fn create() -> Result<Self, &'static str> {
        let config = ffi::PyInterpreterConfig {
            use_main_obmalloc: 0,
            allow_fork: 0,
            allow_exec: 0,
            allow_threads: 0,
            allow_daemon_threads: 0,
            check_multi_interp_extensions: 1,
            gil: ffi::PyInterpreterConfig_OWN_GIL,
        };
        unsafe {
            let gil = ffi::PyGILState_Ensure();
            let main = ffi::PyThreadState_Get();
            let mut tstate: *mut ffi::PyThreadState = std::ptr::null_mut();
            let status = ffi::Py_NewInterpreterFromConfig(&mut tstate, &config);
            let failed = ffi::PyStatus_Exception(status) != 0;
            if !failed {
                ffi::PyThreadState_Swap(main);
            }
            ffi::PyGILState_Release(gil);
            if failed {
                return Err("could not create subinterpreter");
            }
            Ok(Self { tstate })
        }
    }

    /// Starts this interpreter's warden. Call on the owning thread.
    pub(super) fn warden(&self) -> Warden {
        let remote = Remote {
            interp: unsafe { ffi::PyThreadState_GetInterpreter(self.tstate) },
            thread: unsafe { PyThread_get_thread_ident() },
        };
        let (commands, receiver) = channel();
        let landed = Arc::new(AtomicUsize::new(0));
        let counter = landed.clone();
        let thread = std::thread::spawn(move || remote.serve(&receiver, &counter));
        Warden {
            commands,
            thread: Some(thread),
            landed,
        }
    }

    /// Runs `f` with this interpreter attached (holding its GIL).
    pub(super) fn attached<T>(&mut self, f: impl FnOnce() -> T) -> T {
        unsafe { ffi::PyEval_RestoreThread(self.tstate) };
        let out = f();
        unsafe { ffi::PyEval_SaveThread() };
        out
    }

    /// Ends the interpreter. With threads disallowed, this thread's is the only
    /// thread state, which `Py_EndInterpreter` requires. An interrupt that arrived
    /// after the bot had already returned is dropped so it cannot fire during
    /// finalization.
    pub(super) fn destroy(self) {
        unsafe {
            ffi::PyEval_RestoreThread(self.tstate);
            ffi::PyThreadState_SetAsyncExc(
                PyThread_get_thread_ident() as c_long,
                std::ptr::null_mut(),
            );
            ffi::Py_EndInterpreter(self.tstate);
        }
    }
}

unsafe extern "C" {
    fn PyThread_get_thread_ident() -> c_ulong;
}

pub(super) enum Command {
    /// Take the bot's GIL and keep it: the bot stops at its next bytecode boundary,
    /// or when the C call it is inside returns.
    Freeze,
    /// Let go of the GIL: the bot carries on where it stopped.
    Thaw,
    Interrupt,
    Stop,
}

/// The one thread besides the bot's own that touches its interpreter. It runs the
/// runner's commands in order; each needs the bot's GIL, so a `Freeze` waits for
/// the bot to hand it over and everything queued behind it waits too.
pub(super) struct Warden {
    commands: Sender<Command>,
    thread: Option<JoinHandle<()>>,
    landed: Arc<AtomicUsize>,
}

impl Warden {
    pub(super) fn freeze(&self) {
        let _ = self.commands.send(Command::Freeze);
    }

    pub(super) fn thaw(&self) {
        let _ = self.commands.send(Command::Thaw);
    }

    pub(super) fn interrupt(&self) {
        let _ = self.commands.send(Command::Interrupt);
    }

    /// How many freezes have actually taken the GIL so far.
    pub(super) fn landed(&self) -> usize {
        self.landed.load(Ordering::Acquire)
    }

    /// Waits for the warden to finish. Only while the bot is idle: its thread state
    /// must be gone before the interpreter ends, and the wait would never end if the
    /// bot held its GIL.
    pub(super) fn stop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A bot's interpreter as the warden sees it.
struct Remote {
    interp: *mut ffi::PyInterpreterState,
    thread: c_ulong,
}

// Raw pointers to an interpreter that outlives every use.
unsafe impl Send for Remote {}

impl Remote {
    fn serve(self, commands: &Receiver<Command>, landed: &AtomicUsize) {
        let mut tstate: Option<*mut ffi::PyThreadState> = None;
        loop {
            match commands.recv() {
                Ok(Command::Freeze) => {
                    tstate = unsafe { self.attach() };
                    landed.fetch_add(1, Ordering::Release);
                }
                Ok(Command::Thaw) => {
                    if let Some(tstate) = tstate.take() {
                        unsafe {
                            ffi::PyThreadState_Clear(tstate);
                            ffi::PyThreadState_DeleteCurrent();
                        }
                    }
                }
                Ok(Command::Interrupt) => {
                    if tstate.is_some() {
                        unsafe {
                            ffi::PyThreadState_SetAsyncExc(
                                self.thread as c_long,
                                ffi::PyExc_SystemExit,
                            )
                        };
                    }
                }
                Ok(Command::Stop) | Err(_) => {
                    if tstate.is_none() {
                        return;
                    }
                    loop {
                        std::thread::park();
                    }
                }
            }
        }
    }

    /// Attaches a thread state of its own, which means taking the bot's GIL.
    unsafe fn attach(&self) -> Option<*mut ffi::PyThreadState> {
        unsafe {
            let tstate = ffi::PyThreadState_New(self.interp);
            if tstate.is_null() {
                return None;
            }
            ffi::PyEval_RestoreThread(tstate);
            Some(tstate)
        }
    }
}

/// A globals dict for running bootstrap code. Must be used, and dropped, while the
/// interpreter is attached.
pub(super) struct Globals(*mut ffi::PyObject);

impl Globals {
    pub(super) fn new() -> Option<Self> {
        let dict = unsafe { ffi::PyDict_New() };
        (!dict.is_null()).then_some(Self(dict))
    }

    pub(super) fn set_str(&self, name: &str, value: &str) {
        let obj = unsafe {
            ffi::PyUnicode_FromStringAndSize(value.as_ptr() as *const c_char, value.len() as _)
        };
        self.set_obj(name, obj);
    }

    pub(super) fn set_int(&self, name: &str, value: u32) {
        let obj = unsafe { ffi::PyLong_FromUnsignedLong(value.into()) };
        self.set_obj(name, obj);
    }

    /// Takes ownership of `obj`.
    pub(super) fn set_obj(&self, name: &str, obj: *mut ffi::PyObject) {
        assert!(!obj.is_null(), "could not create value for {name}");
        let name = CString::new(name).expect("no NUL");
        unsafe {
            let rc = ffi::PyDict_SetItemString(self.0, name.as_ptr(), obj);
            ffi::Py_DecRef(obj);
            assert_eq!(rc, 0, "could not set global");
        }
    }

    pub(super) fn run(&self, code: &CStr) -> Result<(), String> {
        let obj = self.exec(code, ffi::Py_file_input)?;
        unsafe { ffi::Py_DecRef(obj) };
        Ok(())
    }

    /// Evaluates an expression that must produce a `str`.
    pub(super) fn eval(&self, code: &CStr) -> Result<String, String> {
        let obj = self.exec(code, ffi::Py_eval_input)?;
        let mut len: ffi::Py_ssize_t = 0;
        let ptr = unsafe { ffi::PyUnicode_AsUTF8AndSize(obj, &mut len) };
        let out = if ptr.is_null() {
            Err(take_error())
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
            Ok(String::from_utf8_lossy(bytes).into_owned())
        };
        unsafe { ffi::Py_DecRef(obj) };
        out
    }

    /// Returns a new reference.
    fn exec(&self, code: &CStr, mode: c_int) -> Result<*mut ffi::PyObject, String> {
        let obj = unsafe { ffi::PyRun_String(code.as_ptr(), mode, self.0, self.0) };
        if obj.is_null() {
            return Err(take_error());
        }
        Ok(obj)
    }
}

impl Drop for Globals {
    fn drop(&mut self) {
        unsafe { ffi::Py_DecRef(self.0) };
    }
}

/// Describes and clears the pending exception as `Type: message`. Reached only when
/// the bootstrap itself fails, which a bot can provoke only by breaking its own
/// interpreter.
fn take_error() -> String {
    unsafe {
        let exc = ffi::PyErr_GetRaisedException();
        if exc.is_null() {
            return "unknown error".to_string();
        }
        let type_name = CStr::from_ptr((*ffi::Py_TYPE(exc)).tp_name).to_string_lossy();
        let text = ffi::PyObject_Str(exc);
        let message = if text.is_null() {
            ffi::PyErr_Clear();
            "unprintable".to_string()
        } else {
            let mut len: ffi::Py_ssize_t = 0;
            let ptr = ffi::PyUnicode_AsUTF8AndSize(text, &mut len);
            let out = if ptr.is_null() {
                ffi::PyErr_Clear();
                "unprintable".to_string()
            } else {
                String::from_utf8_lossy(std::slice::from_raw_parts(ptr as *const u8, len as usize))
                    .into_owned()
            };
            ffi::Py_DecRef(text);
            out
        };
        ffi::Py_DecRef(exc);
        format!("{type_name}: {message}")
    }
}
