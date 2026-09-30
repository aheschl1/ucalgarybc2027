//! One bot's interpreter. `init` starts CPython and imports `ucbc._guest`; the build
//! snapshots the instance right after it (`ucbc-dev snapshot`). The engine then calls
//! `compile` once per team, `load` once per bot and `step` per turn. `_host.call` is the bot's one way out: a message to
//! the engine, answered with a reply.
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};

use pyo3_ffi::*;

#[link(wasm_import_module = "ucbc")]
unsafe extern "C" {
    /// Sends a message and returns the length of the reply, which `take` copies out.
    #[link_name = "call"]
    fn host_call(message: *const c_char, len: i32) -> i32;
    #[link_name = "take"]
    fn host_take(reply: *mut c_char);
}

/// `ucbc._guest`, imported by `init`.
static GUEST: AtomicPtr<PyObject> = AtomicPtr::new(null_mut());

unsafe extern "C" fn call(_: *mut PyObject, message: *mut PyObject) -> *mut PyObject {
    unsafe {
        let mut bytes = null_mut();
        let mut len = 0;
        if PyBytes_AsStringAndSize(message, &mut bytes, &mut len) < 0 {
            return null_mut();
        }
        let n = host_call(bytes, len as i32);
        let reply = PyBytes_FromStringAndSize(std::ptr::null(), n as Py_ssize_t);
        if !reply.is_null() {
            host_take(PyBytes_AsString(reply));
        }
        reply
    }
}

static mut HOST_METHODS: [PyMethodDef; 2] = [
    PyMethodDef {
        ml_name: c"call".as_ptr(),
        ml_meth: PyMethodDefPointer { PyCFunction: call },
        ml_flags: METH_O,
        ml_doc: std::ptr::null(),
    },
    PyMethodDef::zeroed(),
];

static mut HOST_MODULE: PyModuleDef = PyModuleDef {
    m_base: PyModuleDef_HEAD_INIT,
    m_name: c"_host".as_ptr(),
    m_doc: std::ptr::null(),
    m_size: -1,
    m_methods: &raw mut HOST_METHODS as *mut PyMethodDef,
    m_slots: null_mut(),
    m_traverse: None,
    m_clear: None,
    m_free: None,
};

unsafe extern "C" fn init_host() -> *mut PyObject {
    unsafe { PyModule_Create(&raw mut HOST_MODULE) }
}

/// The stdlib zip is mounted at /lib (home "/") and the `ucbc` package at /ucbc; the package
/// is imported by path because its parent directory is not mounted.
const IMPORT_UCBC: &CStr = c"import importlib.util, sys
spec = importlib.util.spec_from_file_location(
    'ucbc', '/ucbc/__init__.py', submodule_search_locations=['/ucbc'])
sys.modules['ucbc'] = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sys.modules['ucbc'])
";

fn check(ok: bool) {
    if !ok {
        unsafe { PyErr_Print() };
        std::process::abort();
    }
}

#[unsafe(export_name = "init")]
pub extern "C" fn init() {
    unsafe {
        PyImport_AppendInittab(c"_host".as_ptr(), Some(init_host));
        let mut config = std::mem::MaybeUninit::<PyConfig>::uninit();
        PyConfig_InitIsolatedConfig(config.as_mut_ptr());
        let mut config = config.assume_init();
        config.site_import = 0;
        config.write_bytecode = 0;
        config.use_hash_seed = 1;
        config.hash_seed = 0;
        let home = ['/' as i32, 0];
        check(
            PyStatus_Exception(PyConfig_SetString(
                &mut config,
                &mut config.home,
                home.as_ptr(),
            )) == 0,
        );
        check(PyStatus_Exception(Py_InitializeFromConfig(&config)) == 0);
        PyConfig_Clear(&mut config);
        check(PyRun_SimpleString(IMPORT_UCBC.as_ptr()) == 0);
        let guest = PyImport_ImportModule(c"ucbc._guest".as_ptr());
        check(!guest.is_null());
        GUEST.store(guest, Ordering::Relaxed);
    }
}

/// `ucbc._guest` reports through `_host.call`; anything that escapes it is printed and
/// the export returns without a report, which the engine treats as a crash.
fn run(result: *mut PyObject) {
    unsafe {
        if result.is_null() {
            PyErr_Print();
        } else {
            Py_DECREF(result);
        }
    }
}

#[unsafe(export_name = "compile")]
pub extern "C" fn compile() {
    let guest = GUEST.load(Ordering::Relaxed);
    run(unsafe { PyObject_CallMethod(guest, c"compile".as_ptr(), std::ptr::null()) });
}

#[unsafe(export_name = "load")]
pub extern "C" fn load() {
    let guest = GUEST.load(Ordering::Relaxed);
    run(unsafe { PyObject_CallMethod(guest, c"load".as_ptr(), std::ptr::null()) });
}

#[unsafe(export_name = "step")]
pub extern "C" fn step(set_index: i32, tick: i32) {
    let guest = GUEST.load(Ordering::Relaxed);
    run(unsafe { PyObject_CallMethod(guest, c"step".as_ptr(), c"ii".as_ptr(), set_index, tick) });
}
