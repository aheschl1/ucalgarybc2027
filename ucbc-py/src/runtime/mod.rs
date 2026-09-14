//! Python teams executed in-process. Each bot is one OS thread with its own copy of
//! the team's `main.py` executed into a fresh namespace and an engine-owned memory
//! dict. Steps are strictly serial: the runner thread sends `Step`, services the
//! bot's queries and actions over a channel while the bot thread runs Python, and
//! returns when the bot reports `Done`.
//!
//! Thread rules: the runner thread attaches only while no bot is mid-step. The bot
//! thread attaches only while loading `main.py` or running a step. `RawGame` detaches
//! while it waits for the runner's reply. Breaking any of these deadlocks the match.

mod api;
mod stdout;
mod thread;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use pyo3::prelude::*;
use pyo3::types::PyModule;
use serde_json::Value;
use ucbc_engine::{ActionError, Bot, BotFailure, QueryError, SpawnCtx, StepCtx, StepResult};

pub use api::{Api, PyActionError, PyQueryError, PySetOver};

enum ToBot {
    Step { set_index: u32, tick: u32 },
    QueryReply(Result<Value, QueryError>),
    ActReply(Result<Value, ActionError>),
    Shutdown,
}

enum FromBot {
    Loaded(Result<(), BotFailure>),
    Query(Value),
    Act(Value),
    Done(StepResult),
}

/// The bot thread's end of the channel pair, shared with its `Api`.
struct Link {
    to_runner: Sender<FromBot>,
    from_runner: Mutex<Receiver<ToBot>>,
}

impl Link {
    fn recv(&self) -> Option<ToBot> {
        self.from_runner.lock().ok()?.recv().ok()
    }
}

/// A Python team: `main.py` compiled once, executed afresh for every bot.
pub struct PyTeam {
    path: PathBuf,
    code: Py<PyAny>,
    game: String,
}

impl PyTeam {
    /// Reads and compiles `dir/main.py`. Must run attached; a failure is the team's
    /// load error.
    pub fn compile(py: Python<'_>, dir: &Path, game: &str) -> Result<Self, BotFailure> {
        let path = dir.join("main.py");
        let source = std::fs::read_to_string(&path).map_err(|e| {
            BotFailure::exception("FileNotFoundError", format!("{}: {e}", path.display()))
        })?;
        let compile = || -> PyResult<Py<PyAny>> {
            let builtins = PyModule::import(py, "builtins")?;
            let code =
                builtins
                    .getattr("compile")?
                    .call1((source, path.to_string_lossy(), "exec"))?;
            Ok(code.unbind())
        };
        let code = compile().map_err(|e| thread::failure_from(py, &e))?;
        Ok(Self {
            path,
            code,
            game: game.to_string(),
        })
    }

    /// Spawns one bot of this team.
    pub fn spawn(self: &Arc<Self>, ctx: &SpawnCtx) -> Result<Box<dyn Bot>, BotFailure> {
        Ok(Box::new(PyBot::spawn(ctx, self.clone())?))
    }
}

pub struct PyBot {
    to_bot: Sender<ToBot>,
    from_bot: Receiver<FromBot>,
    thread: Option<JoinHandle<()>>,
}

impl PyBot {
    /// Starts the bot thread and runs `main.py`. A failure is returned here and no
    /// thread is left behind.
    fn spawn(ctx: &SpawnCtx, team: Arc<PyTeam>) -> Result<Self, BotFailure> {
        let (to_bot, from_runner) = channel();
        let (to_runner, from_bot) = channel();
        let link = Arc::new(Link {
            to_runner,
            from_runner: Mutex::new(from_runner),
        });
        let spawn = thread::BotSpawn::new(ctx, team);
        let thread = std::thread::Builder::new()
            .name(format!("bot-{}", ctx.bot.id))
            .spawn(move || thread::run(spawn, link))
            .map_err(|e| BotFailure::Crash(format!("could not start bot thread: {e}")))?;
        let mut bot = Self {
            to_bot,
            from_bot,
            thread: Some(thread),
        };
        match bot.from_bot.recv() {
            Ok(FromBot::Loaded(Ok(()))) => Ok(bot),
            Ok(FromBot::Loaded(Err(failure))) => {
                bot.shutdown();
                Err(failure)
            }
            _ => {
                bot.shutdown();
                Err(BotFailure::Crash("bot thread exited while loading".into()))
            }
        }
    }
}

impl Bot for PyBot {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let sent = self.to_bot.send(ToBot::Step {
            set_index: ctx.set_index,
            tick: ctx.tick,
        });
        if sent.is_err() {
            return StepResult::failed(BotFailure::Crash("bot thread is gone".into()));
        }
        loop {
            match self.from_bot.recv() {
                Ok(FromBot::Query(q)) => {
                    let _ = self.to_bot.send(ToBot::QueryReply(ctx.query(&q)));
                }
                Ok(FromBot::Act(a)) => {
                    let _ = self.to_bot.send(ToBot::ActReply(ctx.act(&a)));
                }
                Ok(FromBot::Done(result)) => return result,
                Ok(FromBot::Loaded(_)) | Err(_) => {
                    return StepResult::failed(BotFailure::Crash(
                        "bot thread exited mid-step".into(),
                    ));
                }
            }
        }
    }

    fn shutdown(&mut self) {
        let _ = self.to_bot.send(ToBot::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
