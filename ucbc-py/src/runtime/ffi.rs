//! Thin wrappers over the parts of the CPython C API this runtime needs: a
//! subinterpreter's lifecycle and running bootstrap code in it. Everything
//! `unsafe` in the runtime lives here and in `bridge`.
//!
//! Nothing in the runtime may use PyO3 while a bot's interpreter is attached: the
//! thread's GIL-state slot is bound to the main interpreter, so `PyGILState_Ensure`
//! would attach to the wrong one.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int};

use pyo3::ffi;

/// A subinterpreter with its own GIL, owned by the calling thread. Not panic-safe:
/// a panic while attached leaves the GIL held.
pub(super) struct Interp {
    tstate: *mut ffi::PyThreadState,
}

impl Interp {
    /// Creates the interpreter. Attaches to the main interpreter only for the call
    /// and leaves the thread detached from everything. `PyGILState_Ensure` must
    /// come first so the thread's GIL-state slot binds to main, not to the new one.
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

    /// Runs `f` with this interpreter attached (holding its GIL).
    pub(super) fn attached<T>(&mut self, f: impl FnOnce() -> T) -> T {
        unsafe { ffi::PyEval_RestoreThread(self.tstate) };
        let out = f();
        unsafe { ffi::PyEval_SaveThread() };
        out
    }

    /// Ends the interpreter. With threads disallowed, this thread's is the only
    /// thread state, which `Py_EndInterpreter` requires.
    pub(super) fn destroy(self) {
        unsafe {
            ffi::PyEval_RestoreThread(self.tstate);
            ffi::Py_EndInterpreter(self.tstate);
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
