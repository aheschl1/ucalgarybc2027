//! Python teams as wasm bots, one `Guest` per bot (`ucbc-wasm`). A guest runs on fuel:
//! `step_ms` is bot time at one fuel unit a nanosecond, so a step that runs out is
//! suspended where it is and resumes on the bot's next turn. Its messages, JSON either
//! way, are answered here; the meter is off meanwhile, so engine time costs it nothing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use ucbc_engine::{ActionError, Bot, BotFailure, SpawnCtx, StepCtx, StepResult};
use ucbc_wasm::{Guest, Outcome, Runtime};

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

impl From<StepReply> for StepResult {
    fn from(reply: StepReply) -> Self {
        match reply.error {
            Some(f) => StepResult::failed(f.into()),
            None => StepResult::ok(),
        }
        .with_stdout(reply.stdout)
    }
}

/// One message from the guest.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FromBot {
    /// The interpreter is up and wants its identity; `main.py` is imported next.
    Ready(()),
    Loaded(Option<Failure>),
    Query(Value),
    Act(Value),
    Done(StepReply),
}

/// A Python team: a directory with `main.py`, imported by each of its bots.
pub struct PyTeam {
    runtime: Arc<Runtime>,
    dir: PathBuf,
    game: String,
}

impl PyTeam {
    pub fn read(runtime: Arc<Runtime>, dir: &Path, game: &str) -> Result<Self, BotFailure> {
        let path = dir.join("main.py");
        if !path.is_file() {
            return Err(BotFailure::exception(
                "FileNotFoundError",
                format!("{}: no such file", path.display()),
            ));
        }
        Ok(Self {
            runtime,
            dir: dir.to_path_buf(),
            game: game.to_string(),
        })
    }

    pub fn spawn(&self, ctx: &SpawnCtx) -> Result<Box<dyn Bot>, BotFailure> {
        Ok(Box::new(PyBot::spawn(ctx, self)?))
    }
}

pub struct PyBot {
    guest: Guest,
    fuel: u64,
}

impl PyBot {
    /// Creates the guest and imports `main.py` on the step budget.
    fn spawn(ctx: &SpawnCtx, team: &PyTeam) -> Result<Self, BotFailure> {
        let mut guest = Guest::new(&team.runtime, &team.dir, ctx.seed, ctx.limits.memory_bytes)
            .map_err(crash)?;
        let identity = json!({ "identity": {
            "bot_id": ctx.bot.id.0,
            "team": ctx.bot.team.0,
            "team_name": ctx.team.name,
            "seed": ctx.seed,
            "game": team.game,
        }});
        let fuel = ctx.limits.step_fuel();
        let mut loaded = None;
        let run = guest
            .load(fuel, |message| match serde_json::from_value(message) {
                Ok(FromBot::Ready(())) => identity.clone(),
                Ok(FromBot::Loaded(failure)) => {
                    loaded = Some(failure);
                    Value::Null
                }
                _ => error("protocol", "not in a step"),
            })
            .map_err(crash)?;
        match (run.outcome, loaded) {
            (Outcome::OutOfFuel, _) => Err(BotFailure::exception(
                "TimeoutError",
                format!("main.py took longer than {} ms to load", ctx.limits.step_ms),
            )),
            (Outcome::Returned, Some(None)) => Ok(Self { guest, fuel }),
            (Outcome::Returned, Some(Some(failure))) => Err(failure.into()),
            (Outcome::Returned, None) => Err(protocol_error()),
        }
    }
}

impl Bot for PyBot {
    /// A suspended step resumes and this turn ends when it returns; otherwise the turn
    /// is a fresh step.
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let mut done = None;
        let run = self
            .guest
            .step(
                ctx.set_index,
                ctx.tick,
                self.fuel,
                |message| match serde_json::from_value(message) {
                    Ok(FromBot::Query(q)) => match ctx.query(&q) {
                        Ok(v) => json!({ "ok": v }),
                        Err(e) => error("query", e),
                    },
                    Ok(FromBot::Act(a)) => match ctx.act(&a) {
                        Ok(v) => json!({ "ok": v }),
                        Err(e @ ActionError::SetOver) => error("set_over", e),
                        Err(e) => error("action", e),
                    },
                    Ok(FromBot::Done(reply)) => {
                        done = Some(reply);
                        Value::Null
                    }
                    _ => error("protocol", "unexpected message"),
                },
            );
        let run = match run {
            Ok(run) => run,
            Err(e) => return StepResult::failed(crash(e)),
        };
        let result = match (run.outcome, done) {
            // Stopped where it is; whatever it already did stands.
            (Outcome::OutOfFuel, _) => StepResult::ok(),
            (Outcome::Returned, Some(reply)) => reply.into(),
            (Outcome::Returned, None) => StepResult::failed(protocol_error()),
        };
        let result = result.with_time(Duration::from_nanos(run.fuel));
        match self.guest.memory_bytes() {
            Some(bytes) => result.with_memory(bytes),
            None => result,
        }
    }
}

/// A trap or a runtime error: the guest is gone.
fn crash(e: impl std::fmt::Display) -> BotFailure {
    BotFailure::Crash(format!("{e:#}"))
}

fn protocol_error() -> BotFailure {
    BotFailure::Crash("bot protocol error".into())
}

fn error(kind: &str, e: impl std::fmt::Display) -> Value {
    json!({ "err": { "kind": kind, "message": e.to_string() } })
}
