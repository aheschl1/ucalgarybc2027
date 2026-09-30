//! Python teams as wasm bots, one `Guest` per bot (`ucbc-wasm`). A guest runs on fuel:
//! `step_ms` is bot time at `FUEL_PER_MS` fuel a millisecond, so a step that runs out is
//! suspended where it is and resumes on the bot's next turn. Its messages, pickled either
//! way, are answered here; the meter is off meanwhile, so engine time costs it nothing.
//! A team's code is compiled to bytecode once, before any bot, so loading it is cheap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_pickle::{DeOptions, SerOptions};
use tempfile::TempDir;
use ucbc_engine::{ActionError, Bot, BotFailure, BotResourceLimit, SpawnCtx, StepCtx, StepResult};
use ucbc_wasm::{FUEL_PER_MS, Guest, Outcome, Runtime, bot_time};

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

/// One reply to the guest. A `done` message is answered with `()`, which pickles as `None`.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ToBot {
    Ok(Value),
    Err { kind: &'static str, message: String },
}

fn decode(message: &[u8]) -> Result<FromBot, serde_pickle::Error> {
    serde_pickle::from_slice(message, DeOptions::new())
}

fn encode(reply: &impl Serialize) -> Vec<u8> {
    serde_pickle::to_vec(reply, SerOptions::new()).expect("a reply pickles")
}

/// Bot time for compiling a team's code, on no bot's budget: a guard, not a limit.
const COMPILE_MS: u64 = 10_000;

/// A Python team: a directory with `main.py`, imported by each of its bots.
pub struct PyTeam {
    runtime: Arc<Runtime>,
    dir: PathBuf,
    cache: TempDir,
    game: String,
}

impl PyTeam {
    pub fn read(
        runtime: Arc<Runtime>,
        dir: &Path,
        game: &str,
        limits: &BotResourceLimit,
    ) -> Result<Self, BotFailure> {
        let path = dir.join("main.py");
        if !path.is_file() {
            return Err(BotFailure::exception(
                "FileNotFoundError",
                format!("{}: no such file", path.display()),
            ));
        }
        let cache = tempfile::tempdir().map_err(crash)?;
        let fuel = COMPILE_MS * FUEL_PER_MS;
        match Guest::compile(&runtime, dir, cache.path(), limits.memory_bytes, fuel)
            .map_err(crash)?
        {
            Outcome::Returned => {}
            Outcome::OutOfFuel => {
                return Err(BotFailure::exception(
                    "TimeoutError",
                    format!("compiling the team took longer than {} ms", COMPILE_MS),
                ));
            }
        }
        Ok(Self {
            runtime,
            dir: dir.to_path_buf(),
            cache,
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
        let mut guest = Guest::new(
            &team.runtime,
            &team.dir,
            team.cache.path(),
            ctx.seed,
            ctx.limits.memory_bytes,
        )
        .map_err(crash)?;
        let identity = encode(&json!({ "identity": {
            "bot_id": ctx.bot.id.0,
            "team": ctx.bot.team.0,
            "team_name": ctx.team.name,
            "seed": ctx.seed,
            "game": team.game,
        }}));
        let fuel = ctx.limits.step_ms * FUEL_PER_MS;
        let mut loaded = None;
        let run = guest
            .load(fuel, |message| match decode(&message) {
                Ok(FromBot::Ready(())) => identity.clone(),
                Ok(FromBot::Loaded(failure)) => {
                    loaded = Some(failure);
                    encode(&())
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
            .step(ctx.set_index, ctx.tick, self.fuel, |message| {
                match decode(&message) {
                    Ok(FromBot::Query(q)) => match ctx.query(&q) {
                        Ok(v) => encode(&ToBot::Ok(v)),
                        Err(e) => error("query", e),
                    },
                    Ok(FromBot::Act(a)) => match ctx.act(&a) {
                        Ok(v) => encode(&ToBot::Ok(v)),
                        Err(e @ ActionError::SetOver) => error("set_over", e),
                        Err(e) => error("action", e),
                    },
                    Ok(FromBot::Done(reply)) => {
                        done = Some(reply);
                        encode(&())
                    }
                    _ => error("protocol", "unexpected message"),
                }
            });
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
        let result = result.with_time(bot_time(run.fuel));
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

fn error(kind: &'static str, e: impl std::fmt::Display) -> Vec<u8> {
    encode(&ToBot::Err {
        kind,
        message: e.to_string(),
    })
}
