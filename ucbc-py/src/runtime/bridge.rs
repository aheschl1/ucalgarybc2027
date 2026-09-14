//! The two C functions a bot's interpreter calls to reach the engine, and the token
//! that gates them.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_void};
use std::sync::{Arc, Mutex};

use pyo3::ffi;
use serde_json::{Value, json};
use ucbc_engine::ActionError;

use super::{FromBot, Link, ToBot};

/// The one way to reach the game during a step. Not `Clone`: the runner creates one
/// per step, the bot thread holds it while the bot runs, and it goes back with `Done`.
/// Outside a step no token exists, so nothing the bot stashed can reach the engine.
pub(super) struct StepToken {
    link: Arc<Link>,
    pub(super) set_index: u32,
    pub(super) tick: u32,
}

impl StepToken {
    pub(super) fn new(link: Arc<Link>, set_index: u32, tick: u32) -> Self {
        Self {
            link,
            set_index,
            tick,
        }
    }
}

/// Per-bot state behind the bridge functions. Lives as long as the interpreter.
#[derive(Default)]
pub(super) struct Bridge {
    token: Mutex<Option<StepToken>>,
}

impl Bridge {
    pub(super) fn begin_step(&self, token: StepToken) {
        *self.token.lock().expect("token lock") = Some(token);
    }

    pub(super) fn end_step(&self) -> Option<StepToken> {
        self.token.lock().expect("token lock").take()
    }

    /// One round trip to the runner, as the JSON reply the Python side expects:
    /// `{"ok": value}` or `{"err": {"kind", "message"}}`.
    fn exchange(&self, msg: FromBot) -> Result<String, &'static str> {
        let guard = self.token.lock().expect("token lock");
        let token = guard
            .as_ref()
            .ok_or("no step in progress for this handle")?;
        let reply = (|| {
            let receiver = token.link.from_runner.lock().ok()?;
            token.link.to_runner.send(msg).ok()?;
            receiver.recv().ok()
        })()
        .ok_or("engine link closed")?;
        let reply = match reply {
            ToBot::QueryReply(Ok(v)) | ToBot::ActReply(Ok(v)) => json!({ "ok": v }),
            ToBot::QueryReply(Err(e)) => error("query", e.to_string()),
            ToBot::ActReply(Err(e @ ActionError::SetOver)) => error("set_over", e.to_string()),
            ToBot::ActReply(Err(e)) => error("action", e.to_string()),
            _ => return Err("engine protocol error"),
        };
        Ok(reply.to_string())
    }
}

fn error(kind: &str, message: String) -> Value {
    json!({ "err": { "kind": kind, "message": message } })
}

const CAPSULE_NAME: &CStr = c"ucbc.bridge";

/// A method definition CPython reads but never writes, so sharing it is sound.
struct MethodDef(ffi::PyMethodDef);

unsafe impl Sync for MethodDef {}

static QUERY_DEF: MethodDef = MethodDef(ffi::PyMethodDef {
    ml_name: c"query".as_ptr(),
    ml_meth: ffi::PyMethodDefPointer { PyCFunction: query },
    ml_flags: ffi::METH_O,
    ml_doc: std::ptr::null(),
});

static ACT_DEF: MethodDef = MethodDef(ffi::PyMethodDef {
    ml_name: c"act".as_ptr(),
    ml_meth: ffi::PyMethodDefPointer { PyCFunction: act },
    ml_flags: ffi::METH_O,
    ml_doc: std::ptr::null(),
});

/// Builtin `query` and `act` functions bound to `bridge`. Must be called with the
/// bot's interpreter attached. Returns new references.
///
/// # Safety
/// `bridge` must outlive the interpreter the functions are installed in.
pub(super) unsafe fn functions(
    bridge: &Bridge,
) -> Result<(*mut ffi::PyObject, *mut ffi::PyObject), ()> {
    let capsule = unsafe {
        ffi::PyCapsule_New(
            bridge as *const Bridge as *mut c_void,
            CAPSULE_NAME.as_ptr(),
            None,
        )
    };
    if capsule.is_null() {
        return Err(());
    }
    let query = unsafe {
        ffi::PyCFunction_NewEx(
            &QUERY_DEF.0 as *const _ as *mut _,
            capsule,
            std::ptr::null_mut(),
        )
    };
    let act = unsafe {
        ffi::PyCFunction_NewEx(
            &ACT_DEF.0 as *const _ as *mut _,
            capsule,
            std::ptr::null_mut(),
        )
    };
    unsafe { ffi::Py_DecRef(capsule) };
    if query.is_null() || act.is_null() {
        return Err(());
    }
    Ok((query, act))
}

unsafe extern "C" fn query(slf: *mut ffi::PyObject, arg: *mut ffi::PyObject) -> *mut ffi::PyObject {
    unsafe { call(slf, arg, FromBot::Query) }
}

unsafe extern "C" fn act(slf: *mut ffi::PyObject, arg: *mut ffi::PyObject) -> *mut ffi::PyObject {
    unsafe { call(slf, arg, FromBot::Act) }
}

unsafe fn call(
    capsule: *mut ffi::PyObject,
    arg: *mut ffi::PyObject,
    wrap: fn(Value) -> FromBot,
) -> *mut ffi::PyObject {
    let bridge = unsafe { ffi::PyCapsule_GetPointer(capsule, CAPSULE_NAME.as_ptr()) };
    if bridge.is_null() {
        return std::ptr::null_mut();
    }
    let bridge = unsafe { &*(bridge as *const Bridge) };
    let mut len: ffi::Py_ssize_t = 0;
    let ptr = unsafe { ffi::PyUnicode_AsUTF8AndSize(arg, &mut len) };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let text = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    let payload = match serde_json::from_slice::<Value>(text) {
        Ok(v) => v,
        Err(e) => return unsafe { raise(ffi::PyExc_ValueError, &format!("invalid JSON: {e}")) },
    };
    match bridge.exchange(wrap(payload)) {
        Ok(reply) => unsafe {
            ffi::PyUnicode_FromStringAndSize(reply.as_ptr() as *const c_char, reply.len() as _)
        },
        Err(msg) => unsafe { raise(ffi::PyExc_RuntimeError, msg) },
    }
}

unsafe fn raise(kind: *mut ffi::PyObject, message: &str) -> *mut ffi::PyObject {
    let message = std::ffi::CString::new(message).unwrap_or_default();
    unsafe { ffi::PyErr_SetString(kind, message.as_ptr()) };
    std::ptr::null_mut()
}
