//! Python teams executed in-process, one subinterpreter per bot. Each bot is one OS
//! thread owning an interpreter with its own GIL, modules, and namespace; the
//! team's `main.py` runs there. Steps are strictly serial: the runner thread sends
//! `Step`, services the bot's queries and actions over a channel while the bot
//! runs Python, and returns when the bot reports `Done`.
//!
//! The runner thread only ever attaches to the main interpreter, and only while no
//! bot is mid-step. A bot thread attaches to its own interpreter only while loading
//! or stepping, and to the main interpreter only to create and destroy its own.

mod bridge;
mod ffi;
mod thread;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use pyo3::prelude::*;
use pyo3::types::PyModule;
use serde_json::Value;
use ucbc_engine::{ActionError, Bot, BotFailure, QueryError, SpawnCtx, StepCtx, StepResult};

use bridge::StepToken;

enum ToBot {
    Step(StepToken),
    QueryReply(Result<Value, QueryError>),
    ActReply(Result<Value, ActionError>),
    Shutdown,
}

enum FromBot {
    Loaded(Result<(), BotFailure>),
    Query(Value),
    Act(Value),
    /// The step's result and its token, returned so the runner owns it again.
    Done(StepResult, StepToken),
}

/// The bot thread's end of the channel pair, shared with its bridge.
struct Link {
    to_runner: Sender<FromBot>,
    from_runner: Mutex<Receiver<ToBot>>,
}

impl Link {
    fn recv(&self) -> Option<ToBot> {
        self.from_runner.lock().ok()?.recv().ok()
    }
}

/// A Python team: `main.py` checked once, executed afresh in every bot's interpreter.
pub struct PyTeam {
    path: PathBuf,
    source: String,
    game: String,
}

impl PyTeam {
    /// Reads `dir/main.py` and checks that it compiles. Must run attached to the
    /// main interpreter; a failure is the team's load error.
    pub fn compile(py: Python<'_>, dir: &Path, game: &str) -> Result<Self, BotFailure> {
        let path = dir.join("main.py");
        let source = std::fs::read_to_string(&path).map_err(|e| {
            BotFailure::exception("FileNotFoundError", format!("{}: {e}", path.display()))
        })?;
        let check = || -> PyResult<()> {
            PyModule::import(py, "builtins")?
                .getattr("compile")?
                .call1((source.as_str(), path.to_string_lossy(), "exec"))?;
            Ok(())
        };
        check().map_err(|e| failure_from(py, &e))?;
        Ok(Self {
            path,
            source,
            game: game.to_string(),
        })
    }

    /// Spawns one bot of this team.
    pub fn spawn(self: &Arc<Self>, ctx: &SpawnCtx) -> Result<Box<dyn Bot>, BotFailure> {
        Ok(Box::new(PyBot::spawn(ctx, self.clone())?))
    }
}

pub struct PyBot {
    link: Arc<Link>,
    to_bot: Sender<ToBot>,
    from_bot: Receiver<FromBot>,
    thread: Option<JoinHandle<()>>,
}

impl PyBot {
    /// Starts the bot thread, which creates the interpreter and runs `main.py`. A
    /// failure is returned here and no thread is left behind.
    fn spawn(ctx: &SpawnCtx, team: Arc<PyTeam>) -> Result<Self, BotFailure> {
        let (to_bot, from_runner) = channel();
        let (to_runner, from_bot) = channel();
        let link = Arc::new(Link {
            to_runner,
            from_runner: Mutex::new(from_runner),
        });
        let spawn = thread::BotSpawn::new(ctx, team);
        let thread_link = link.clone();
        let thread = std::thread::Builder::new()
            .name(format!("bot-{}", ctx.bot.id))
            .spawn(move || thread::run(spawn, thread_link))
            .map_err(|e| BotFailure::Crash(format!("could not start bot thread: {e}")))?;
        let mut bot = Self {
            link,
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
        let token = StepToken::new(self.link.clone(), ctx.set_index, ctx.tick);
        if self.to_bot.send(ToBot::Step(token)).is_err() {
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
                Ok(FromBot::Done(result, _token)) => return result,
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

pub(super) fn failure_from(py: Python<'_>, err: &PyErr) -> BotFailure {
    let kind = err
        .get_type(py)
        .qualname()
        .map(|q| q.to_string())
        .unwrap_or_else(|_| "Exception".into());
    let message = err
        .value(py)
        .str()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let traceback = err
        .traceback(py)
        .and_then(|tb| tb.format().ok())
        .unwrap_or_default();
    BotFailure::exception(kind, message).with_traceback(traceback)
}
