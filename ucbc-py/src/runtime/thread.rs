//! The bot thread: one subinterpreter, `ucbc._bootstrap` loaded into it, then one
//! step per `Step` message. The bootstrap speaks JSON so nothing here touches
//! Python objects; PyO3 objects cannot live in a subinterpreter.

#![allow(unsafe_code)]

use std::ffi::CString;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;
use ucbc_engine::{BotFailure, BotRef, SpawnCtx, StepResult};

use super::bridge::{self, Bridge};
use super::ffi::{Globals, Interp};
use super::{FromBot, Link, PyTeam, ToBot};

/// Everything the thread needs, copied out of the spawn context.
pub(super) struct BotSpawn {
    bot: BotRef,
    team_name: String,
    seed: u64,
    team: Arc<PyTeam>,
}

impl BotSpawn {
    pub(super) fn new(ctx: &SpawnCtx, team: Arc<PyTeam>) -> Self {
        Self {
            bot: ctx.bot,
            team_name: ctx.team.name.clone(),
            seed: ctx.seed,
            team,
        }
    }
}

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
struct LoadReply {
    error: Option<Failure>,
}

#[derive(Deserialize)]
struct StepReply {
    stdout: String,
    error: Option<Failure>,
}

pub(super) fn run(spawn: BotSpawn, link: Arc<Link>) {
    let bridge = Box::new(Bridge::default());
    let mut interp = match Interp::create() {
        Ok(interp) => interp,
        Err(msg) => {
            let _ = link
                .to_runner
                .send(FromBot::Loaded(Err(BotFailure::Crash(msg.into()))));
            return;
        }
    };

    let loaded = interp.attached(|| load(&spawn, &bridge));
    let outcome = loaded.as_ref().map(|_| ()).map_err(Clone::clone);
    if link.to_runner.send(FromBot::Loaded(outcome)).is_err() || loaded.is_err() {
        interp.destroy();
        return;
    }

    while let Some(ToBot::Step(token)) = link.recv() {
        let (set_index, tick) = (token.set_index, token.tick);
        bridge.begin_step(token);
        let result = interp.attached(|| run_step(set_index, tick));
        let token = bridge.end_step().expect("token was slotted in above");
        if link.to_runner.send(FromBot::Done(result, token)).is_err() {
            break;
        }
    }
    interp.destroy();
    drop(bridge);
}

/// Runs `ucbc._bootstrap.load` in the interpreter.
fn load(spawn: &BotSpawn, bridge: &Bridge) -> Result<(), BotFailure> {
    let identity = json!({
        "bot_id": spawn.bot.id.0,
        "team": spawn.bot.team.0,
        "team_name": spawn.team_name,
        "seed": spawn.seed,
        "game": spawn.team.game,
    })
    .to_string();
    let (query, act) = unsafe { bridge::functions(bridge) }
        .map_err(|()| BotFailure::Crash("could not create bridge functions".into()))?;
    let globals = Globals::new().ok_or_else(|| BotFailure::Crash("no globals".into()))?;
    globals.set_str("SOURCE", &spawn.team.source);
    globals.set_str("PATH", &spawn.team.path.to_string_lossy());
    globals.set_str("IDENTITY", &identity);
    globals.set_obj("QUERY", query);
    globals.set_obj("ACT", act);
    globals
        .run(c"import ucbc._bootstrap as _b")
        .map_err(|e| BotFailure::Crash(format!("bootstrap import failed: {e}")))?;
    let reply = globals
        .eval(c"_b.load(SOURCE, PATH, IDENTITY, QUERY, ACT)")
        .map_err(|e| BotFailure::Crash(format!("bootstrap load failed: {e}")))?;
    let reply: LoadReply = serde_json::from_str(&reply)
        .map_err(|e| BotFailure::Crash(format!("bad load reply: {e}")))?;
    match reply.error {
        Some(f) => Err(f.into()),
        None => Ok(()),
    }
}

/// Runs `ucbc._bootstrap.run_step` in the interpreter.
fn run_step(set_index: u32, tick: u32) -> StepResult {
    let Some(globals) = Globals::new() else {
        return StepResult::failed(BotFailure::Crash("no globals".into()));
    };
    let code = CString::new(format!(
        "__import__('ucbc._bootstrap')._bootstrap.run_step({set_index}, {tick})"
    ))
    .expect("no NUL");
    let reply = match globals.eval(&code) {
        Ok(reply) => reply,
        Err(e) => {
            return StepResult::failed(BotFailure::Crash(format!("bootstrap step failed: {e}")));
        }
    };
    let reply: StepReply = match serde_json::from_str(&reply) {
        Ok(r) => r,
        Err(e) => return StepResult::failed(BotFailure::Crash(format!("bad step reply: {e}"))),
    };
    match reply.error {
        Some(f) => StepResult::failed(f.into()).with_stdout(reply.stdout),
        None => StepResult::ok().with_stdout(reply.stdout),
    }
}
