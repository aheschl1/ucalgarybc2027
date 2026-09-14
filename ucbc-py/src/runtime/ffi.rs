//! Thin, safe-to-use wrappers over the parts of the CPython C API this runtime
//! needs: a subinterpreter's lifecycle and running bootstrap code in it. Everything
//! `unsafe` in the runtime lives here and in `bridge`.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int};

use pyo3::ffi;

/// A subinterpreter with its own GIL, owned by the calling thread.
pub(super) struct Interp {
    tstate: *mut ffi::PyThreadState,
}

impl Interp {
    /// Creates the interpreter. Attaches to the main interpreter only for the call
    /// and leaves the thread detached from everything.
    pub(super) fn create() -> Result<Self, &'static str> {
        let config = ffi::PyInterpreterConfig {
            use_main_obmalloc: 0,
            allow_fork: 0,
            allow_exec: 0,
            allow_threads: 1,
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
                // The new interpreter is current; go back to main so the GIL state
                // bookkeeping balances, then release main.
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

    pub(super) fn destroy(self) {
        unsafe {
            ffi::PyEval_RestoreThread(self.tstate);
            ffi::Py_EndInterpreter(self.tstate);
        }
    }
}

/// A globals dict for running bootstrap code. Must be used while attached.
pub(super) struct Globals(*mut ffi::PyObject);

impl Globals {
    pub(super) fn new() -> Option<Self> {
        unsafe {
            let dict = ffi::PyDict_New();
            if dict.is_null() {
                return None;
            }
            ffi::PyDict_SetItemString(dict, c"__builtins__".as_ptr(), ffi::PyEval_GetBuiltins());
            Some(Self(dict))
        }
    }

    pub(super) fn set_str(&self, name: &str, value: &str) {
        let name = CString::new(name).expect("no NUL");
        unsafe {
            let obj =
                ffi::PyUnicode_FromStringAndSize(value.as_ptr() as *const c_char, value.len() as _);
            ffi::PyDict_SetItemString(self.0, name.as_ptr(), obj);
            ffi::Py_DecRef(obj);
        }
    }

    /// Takes ownership of `obj`.
    pub(super) fn set_obj(&self, name: &str, obj: *mut ffi::PyObject) {
        let name = CString::new(name).expect("no NUL");
        unsafe {
            ffi::PyDict_SetItemString(self.0, name.as_ptr(), obj);
            ffi::Py_DecRef(obj);
        }
    }

    pub(super) fn run(&self, code: &CStr) -> Result<(), String> {
        self.exec(code, ffi::Py_file_input).map(|_| ())
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

    fn exec(&self, code: &CStr, mode: c_int) -> Result<*mut ffi::PyObject, String> {
        let obj = unsafe { ffi::PyRun_String(code.as_ptr(), mode, self.0, self.0) };
        if obj.is_null() {
            return Err(take_error());
        }
        unsafe {
            if mode == ffi::Py_file_input {
                ffi::Py_DecRef(obj);
            }
        }
        Ok(obj)
    }
}

impl Drop for Globals {
    fn drop(&mut self) {
        unsafe { ffi::Py_DecRef(self.0) };
    }
}

/// Describes and clears the pending exception. Used only for bootstrap failures,
/// which are bugs, not bot errors.
fn take_error() -> String {
    unsafe {
        let exc = ffi::PyErr_GetRaisedException();
        if exc.is_null() {
            return "unknown error".to_string();
        }
        let text = ffi::PyObject_Str(exc);
        let mut len: ffi::Py_ssize_t = 0;
        let ptr = if text.is_null() {
            std::ptr::null()
        } else {
            ffi::PyUnicode_AsUTF8AndSize(text, &mut len)
        };
        let out = if ptr.is_null() {
            "unprintable error".to_string()
        } else {
            String::from_utf8_lossy(std::slice::from_raw_parts(ptr as *const u8, len as usize))
                .into_owned()
        };
        ffi::Py_DecRef(text);
        ffi::Py_DecRef(exc);
        out
    }
}
