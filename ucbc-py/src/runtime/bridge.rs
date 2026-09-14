//! The bot thread's end of the channel to the runner, the C functions a bot's
//! interpreter calls to reach it, and the token that gates them.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::sync::mpsc::{Receiver, Sender};

use pyo3::ffi;
use serde_json::{Value, json};
use ucbc_engine::ActionError;

use super::{FromBot, ToBot};

/// The one way to reach the game during a step. Not `Clone`: the runner creates one
/// per step, the bridge holds it while the bot runs, and it goes back with `Done`.
/// Outside a step no token exists, so nothing the bot stashed can reach the engine.
pub(super) struct StepToken {
    pub(super) set_index: u32,
    pub(super) tick: u32,
}

impl StepToken {
    pub(super) fn new(set_index: u32, tick: u32) -> Self {
        Self { set_index, tick }
    }
}

/// The bot thread's channel pair.
pub(super) struct Link {
    to_runner: Sender<FromBot>,
    from_runner: Receiver<ToBot>,
}

impl Link {
    pub(super) fn new(to_runner: Sender<FromBot>, from_runner: Receiver<ToBot>) -> Self {
        Self {
            to_runner,
            from_runner,
        }
    }
}

/// Per-bot state behind the bridge functions. Owned by the bot thread; the C
/// functions reach it through a capsule. Round trips hold the interpreter's GIL,
/// which is fine because stepping is serial.
pub(super) struct Bridge {
    link: Link,
    token: RefCell<Option<StepToken>>,
}

impl Bridge {
    pub(super) fn new(link: Link) -> Self {
        Self {
            link,
            token: RefCell::new(None),
        }
    }

    pub(super) fn send(&self, msg: FromBot) -> bool {
        self.link.to_runner.send(msg).is_ok()
    }

    pub(super) fn recv(&self) -> Option<ToBot> {
        self.link.from_runner.recv().ok()
    }

    pub(super) fn begin_step(&self, token: StepToken) {
        *self.token.borrow_mut() = Some(token);
    }

    pub(super) fn end_step(&self) -> Option<StepToken> {
        self.token.borrow_mut().take()
    }

    /// One round trip to the runner, as the JSON reply the Python side expects:
    /// `{"ok": value}` or `{"err": {"kind", "message"}}`.
    fn exchange(&self, msg: FromBot) -> Result<String, &'static str> {
        let token = self.token.try_borrow().map_err(|_| "handle is busy")?;
        if token.is_none() {
            return Err("no step in progress for this handle");
        }
        if !self.send(msg) {
            return Err("engine link closed");
        }
        let reply = match self.recv().ok_or("engine link closed")? {
            ToBot::StepOver => return Err("the step is over"),
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

const fn method(name: &'static CStr, f: ffi::PyCFunction) -> MethodDef {
    MethodDef(ffi::PyMethodDef {
        ml_name: name.as_ptr(),
        ml_meth: ffi::PyMethodDefPointer { PyCFunction: f },
        ml_flags: ffi::METH_O,
        ml_doc: std::ptr::null(),
    })
}

/// Every function the bot's interpreter gets, by the name Python sees.
static TABLE: [MethodDef; 2] = [method(c"query", query), method(c"act", act)];

/// A dict of the bridge functions bound to `bridge`. Must be called with the bot's
/// interpreter attached. Returns a new reference.
///
/// # Safety
/// `bridge` must outlive the interpreter the functions are installed in.
pub(super) unsafe fn functions(bridge: &Bridge) -> Result<*mut ffi::PyObject, ()> {
    unsafe {
        let capsule = ffi::PyCapsule_New(
            bridge as *const Bridge as *mut c_void,
            CAPSULE_NAME.as_ptr(),
            None,
        );
        if capsule.is_null() {
            return Err(());
        }
        let dict = ffi::PyDict_New();
        let mut ok = !dict.is_null();
        for def in &TABLE {
            if !ok {
                break;
            }
            let f =
                ffi::PyCFunction_NewEx(&def.0 as *const _ as *mut _, capsule, std::ptr::null_mut());
            ok = !f.is_null() && ffi::PyDict_SetItemString(dict, def.0.ml_name, f) == 0;
            ffi::Py_DecRef(f);
        }
        ffi::Py_DecRef(capsule);
        if ok {
            Ok(dict)
        } else {
            ffi::Py_DecRef(dict);
            Err(())
        }
    }
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
