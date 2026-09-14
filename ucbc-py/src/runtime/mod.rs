//! Python teams executed in-process, one subinterpreter per bot. Each bot is one OS
//! thread owning an interpreter with its own GIL, modules, and namespace; the
//! team's `main.py` runs there. Steps are strictly serial: the runner thread sends
//! `Step`, services the bot's queries and actions over a channel while the bot
//! runs Python, and returns when the bot reports `Done`.
//!
//! The runner thread never attaches to any interpreter. A bot thread attaches to
//! its own interpreter only while loading or stepping, and to the main interpreter
//! only to create and destroy its own. The only other thread that touches a bot's
//! interpreter is its warden, which freezes and thaws it on the runner's behalf.
//!
//! The game's limits are enforced here: memory through allocator hooks on the bot
//! thread, time by the runner, which freezes a bot past its budget and lets it
//! carry on at its next turn.

#![allow(unsafe_code)]

mod bridge;
mod ffi;
mod memory;
mod thread;

use std::os::unix::thread::JoinHandleExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ucbc_engine::{ActionError, Bot, BotFailure, QueryError, SpawnCtx, StepCtx, StepResult};

use bridge::StepToken;
use ffi::Warden;

/// How long a bot frozen at the end of a set gets to unwind `SystemExit`.
const UNWIND: Duration = Duration::from_secs(1);

/// Bots abandoned mid-step; their threads and interpreters outlive the match.
static ABANDONED: AtomicUsize = AtomicUsize::new(0);

pub fn abandoned_bots() -> usize {
    ABANDONED.load(Ordering::Relaxed)
}

/// Installs the process-wide memory hooks. Must hold the main GIL.
pub fn install() -> Result<(), &'static str> {
    memory::install()
}

enum ToBot {
    Step(StepToken),
    QueryReply(Result<Value, QueryError>),
    ActReply(Result<Value, ActionError>),
    /// The set is over; the call is refused.
    StepOver,
    Shutdown,
}

enum FromBot {
    /// The interpreter is up, with its warden, or could not be.
    Started(Result<Warden, BotFailure>),
    Loaded(Result<(), BotFailure>),
    Query(Value),
    Act(Value),
    /// The step's result and its token, returned so the runner owns it again.
    Done(StepResult, StepToken),
}

/// A Python team: `main.py` read once, executed afresh in every bot's interpreter.
pub struct PyTeam {
    path: PathBuf,
    source: String,
    game: String,
}

impl PyTeam {
    pub fn read(dir: &Path, game: &str) -> Result<Self, BotFailure> {
        let path = dir.join("main.py");
        let source = std::fs::read_to_string(&path).map_err(|e| {
            BotFailure::exception("FileNotFoundError", format!("{}: {e}", path.display()))
        })?;
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
    to_bot: Sender<ToBot>,
    from_bot: Receiver<FromBot>,
    /// `None` once the thread is dead or abandoned.
    thread: Option<JoinHandle<()>>,
    warden: Warden,
    /// The bot ran out of time and stopped mid-work; its next turn resumes it.
    frozen: bool,
    /// Freezes sent, against the warden's count of those that took the GIL.
    freezes: usize,
    step_time: Duration,
}

impl PyBot {
    /// Starts the bot thread, which creates the interpreter and runs `main.py` within
    /// the step budget; a load that overruns is frozen like a step. A failure is
    /// returned here and no thread is left behind.
    fn spawn(ctx: &SpawnCtx, team: Arc<PyTeam>) -> Result<Self, BotFailure> {
        let (to_bot, from_runner) = channel();
        let (to_runner, from_bot) = channel();
        let identity = json!({
            "bot_id": ctx.bot.id.0,
            "team": ctx.bot.team.0,
            "team_name": ctx.team.name,
            "seed": ctx.seed,
            "game": team.game,
        })
        .to_string();
        let link = bridge::Link::new(to_runner, from_runner);
        let memory = ctx.limits.memory_bytes;
        let thread = std::thread::Builder::new()
            .name(format!("bot-{}", ctx.bot.id))
            .spawn(move || thread::run(identity, team, link, memory))
            .map_err(|e| BotFailure::Crash(format!("could not start bot thread: {e}")))?;
        let started = from_bot.recv();
        let Ok(FromBot::Started(Ok(warden))) = started else {
            let _ = thread.join();
            return Err(match started {
                Ok(FromBot::Started(Err(failure))) => failure,
                _ => BotFailure::Crash("bot thread exited while starting".into()),
            });
        };
        let mut bot = Self {
            to_bot,
            from_bot,
            thread: Some(thread),
            warden,
            frozen: false,
            freezes: 0,
            step_time: ctx.limits.step_time(),
        };
        match bot.wait(None, bot.step_time).outcome {
            Ok(()) => Ok(bot),
            Err(failure) => {
                bot.shutdown();
                Err(failure)
            }
        }
    }

    /// Lets the bot run until it reports back or the budget ends, servicing its
    /// engine calls through `ctx` meanwhile; the time those take is the engine's and
    /// extends the deadline. Without `ctx`, reached only while a set ends, calls are
    /// refused. At the deadline the bot is frozen and the step ends with nothing
    /// more than it already did.
    ///
    /// A freeze lands only when the bot reaches a bytecode boundary, which a C call
    /// can delay. A bot that stayed silent for a whole turn while the previous freeze
    /// never landed is inside one that will not return, and is abandoned.
    fn wait(&mut self, mut ctx: Option<&mut StepCtx<'_>>, budget: Duration) -> StepResult {
        let started = Instant::now();
        let mut heard = false;
        loop {
            let served = ctx.as_ref().map_or(Duration::ZERO, |ctx| ctx.engine_time());
            let left = (started + budget + served).saturating_duration_since(Instant::now());
            let message = match self.from_bot.recv_timeout(left) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => {
                    let stuck = !heard && self.warden.landed() < self.freezes;
                    self.warden.freeze();
                    self.freezes += 1;
                    self.frozen = true;
                    if !stuck {
                        return StepResult::ok();
                    }
                    self.abandon();
                    return StepResult::failed(BotFailure::Crash(
                        "did not stop within a step of the time limit".into(),
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.thread = None;
                    return StepResult::failed(BotFailure::Crash("bot thread exited".into()));
                }
            };
            heard = true;
            match (message, &mut ctx) {
                (FromBot::Query(q), Some(ctx)) => {
                    let _ = self.to_bot.send(ToBot::QueryReply(ctx.query(&q)));
                }
                (FromBot::Act(a), Some(ctx)) => {
                    let _ = self.to_bot.send(ToBot::ActReply(ctx.act(&a)));
                }
                (FromBot::Query(_) | FromBot::Act(_), None) => {
                    let _ = self.to_bot.send(ToBot::StepOver);
                }
                // The load overran into this turn; now the step itself.
                (FromBot::Loaded(Ok(())), Some(ctx)) => {
                    let token = StepToken::new(ctx.set_index, ctx.tick);
                    let _ = self.to_bot.send(ToBot::Step(token));
                }
                (FromBot::Loaded(Ok(())), None) => return StepResult::ok(),
                (FromBot::Loaded(Err(failure)), _) => return StepResult::failed(failure),
                (FromBot::Done(result, _token), _) => return result,
                (FromBot::Started(_), _) => {
                    return StepResult::failed(BotFailure::Crash("bot protocol error".into()));
                }
            }
        }
    }

    /// Gives up on the thread: it stays frozen until the process exits, and until
    /// its C call returns it runs at idle priority. Never joined, never told to shut
    /// down.
    fn abandon(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        let idle = libc::sched_param { sched_priority: 0 };
        unsafe { libc::pthread_setschedparam(thread.as_pthread_t(), libc::SCHED_IDLE, &idle) };
        ABANDONED.fetch_add(1, Ordering::Relaxed);
    }

    fn thaw(&mut self) {
        self.warden.thaw();
        self.frozen = false;
    }
}

impl Bot for PyBot {
    /// A frozen bot picks up where it stopped and this turn ends when that work
    /// returns; otherwise the turn is a fresh step.
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        if self.frozen {
            self.thaw();
        } else {
            let token = StepToken::new(ctx.set_index, ctx.tick);
            if self.to_bot.send(ToBot::Step(token)).is_err() {
                return StepResult::failed(BotFailure::Crash("bot thread is gone".into()));
            }
        }
        self.wait(Some(ctx), self.step_time)
    }

    /// A frozen bot is unwound with `SystemExit` first; one that does not return is
    /// abandoned. The warden stops before the interpreter ends, and is never joined
    /// once the bot thread is gone, since it may be waiting on that thread's GIL.
    fn shutdown(&mut self) {
        if self.thread.is_none() {
            return;
        }
        if self.frozen {
            self.warden.interrupt();
            self.thaw();
            self.wait(None, UNWIND);
            if self.frozen {
                return self.abandon();
            }
        }
        let Some(thread) = self.thread.take() else {
            return;
        };
        self.warden.stop();
        let _ = self.to_bot.send(ToBot::Shutdown);
        let _ = thread.join();
    }
}
