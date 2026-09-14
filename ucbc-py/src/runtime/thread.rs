//! The bot thread: one subinterpreter, `ucbc._bootstrap` loaded into it, then one
//! step per `Step` message. The bootstrap speaks JSON so nothing here touches
//! Python objects; PyO3 objects cannot live in a subinterpreter.

#![allow(unsafe_code)]

use std::sync::Arc;

use serde::Deserialize;
use ucbc_engine::{BotFailure, StepResult};

use super::bridge::{self, Bridge, Link};
use super::ffi::{Globals, Interp};
use super::{FromBot, PyTeam, ToBot};

#[derive(Deserialize)]
struct Failure {
    kind: String,
    message: String,
    traceback: String,
}

impl From<Failure> for BotFailure {
    fn from(f: Failure) -> Self {
        BotFailure::exception(f.kind, f.message).with_traceback(f.traceback)
    }
}

#[derive(Deserialize)]
struct StepReply {
    stdout: String,
    error: Option<Failure>,
}

pub(super) fn run(identity: String, team: Arc<PyTeam>, link: Link) {
    let bridge = Bridge::new(link);
    let mut interp = match Interp::create() {
        Ok(interp) => interp,
        Err(msg) => {
            bridge.send(FromBot::Loaded(Err(BotFailure::Crash(msg.into()))));
            return;
        }
    };

    let loaded = interp.attached(|| load(&identity, &team, &bridge));
    let outcome = loaded.as_ref().map(|_| ()).map_err(Clone::clone);
    let Ok(globals) = loaded else {
        bridge.send(FromBot::Loaded(outcome));
        interp.destroy();
        return;
    };
    if bridge.send(FromBot::Loaded(outcome)) {
        while let Some(ToBot::Step(token)) = bridge.recv() {
            let (set_index, tick) = (token.set_index, token.tick);
            bridge.begin_step(token);
            let result = interp.attached(|| run_step(&globals, set_index, tick));
            let token = bridge.end_step().expect("token was slotted in above");
            if !bridge.send(FromBot::Done(result, token)) {
                break;
            }
        }
    }
    interp.attached(|| drop(globals));
    interp.destroy();
}

/// Imports the bootstrap and runs `load` in the interpreter. The returned globals
/// hold the bootstrap module for the steps.
fn load(identity: &str, team: &PyTeam, bridge: &Bridge) -> Result<Globals, BotFailure> {
    let crash = |what: &str, e: String| BotFailure::Crash(format!("bootstrap {what}: {e}"));
    let functions = unsafe { bridge::functions(bridge) }
        .map_err(|()| BotFailure::Crash("could not create bridge functions".into()))?;
    let globals = Globals::new().ok_or_else(|| BotFailure::Crash("no globals".into()))?;
    globals.set_str("SOURCE", &team.source);
    globals.set_str("PATH", &team.path.to_string_lossy());
    globals.set_str("IDENTITY", identity);
    globals.set_obj("BRIDGE", functions);
    globals
        .run(c"import ucbc._bootstrap as _b")
        .map_err(|e| crash("import failed", e))?;
    let reply = globals
        .eval(c"_b.load(SOURCE, PATH, IDENTITY, BRIDGE)")
        .map_err(|e| crash("load failed", e))?;
    let failure: Option<Failure> =
        serde_json::from_str(&reply).map_err(|e| crash("bad load reply", e.to_string()))?;
    match failure {
        Some(f) => Err(f.into()),
        None => Ok(globals),
    }
}

/// Runs `run_step` in the interpreter.
fn run_step(globals: &Globals, set_index: u32, tick: u32) -> StepResult {
    let crash = |what: &str, e: String| {
        StepResult::failed(BotFailure::Crash(format!("bootstrap {what}: {e}")))
    };
    globals.set_int("SET_INDEX", set_index);
    globals.set_int("TICK", tick);
    let reply = match globals.eval(c"_b.run_step(SET_INDEX, TICK)") {
        Ok(reply) => reply,
        Err(e) => return crash("step failed", e),
    };
    let reply: StepReply = match serde_json::from_str(&reply) {
        Ok(r) => r,
        Err(e) => return crash("bad step reply", e.to_string()),
    };
    match reply.error {
        Some(f) => StepResult::failed(f.into()).with_stdout(reply.stdout),
        None => StepResult::ok().with_stdout(reply.stdout),
    }
}
