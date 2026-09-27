//! Python teams as child processes, one per bot: `python -m ucbc._bot`, spoken to
//! over its stdin and stdout in line-delimited JSON. The process is the unit of
//! isolation and of enforcement. A bot past its step budget is stopped with
//! `SIGSTOP` and resumed at its next turn with `SIGCONT`; at despawn it is killed.
//! Its memory is bounded by `RLIMIT_AS`, which it sets on itself before loading
//! `main.py`, and it locks itself out of signalling, forking, and exec with a seccomp
//! filter (`lockdown` in `lib.rs`).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ucbc_engine::{ActionError, Bot, BotFailure, SpawnCtx, StepCtx, StepResult};

/// How long a bot process gets to start Python and import the SDK. Engine time, not
/// the bot's: its load budget starts when it reports `Ready`.
const STARTUP: Duration = Duration::from_secs(10);

// Only interpreter setup needed by source installs; never inherit worker credentials.
const BOT_ENV_ALLOWLIST: &[&str] = &["PYTHONPATH"];

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
    memory: Option<u64>,
}

impl From<StepReply> for StepResult {
    fn from(reply: StepReply) -> Self {
        let result = match reply.error {
            Some(f) => StepResult::failed(f.into()),
            None => StepResult::ok(),
        };
        let result = result.with_stdout(reply.stdout);
        match reply.memory {
            Some(bytes) => result.with_memory(bytes),
            None => result,
        }
    }
}

/// One line from the bot process.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FromBot {
    /// Python is up and the SDK imported; `main.py` runs next.
    Ready(()),
    Loaded(Option<Failure>),
    Query(Value),
    Act(Value),
    Done(StepReply),
}

/// One line to it.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ToBot {
    Step { set_index: u32, tick: u32 },
    Reply(Value),
}

/// A line from the bot, or why it could not be read as one.
type Message = Result<FromBot, String>;

/// A Python team: `main.py` read once, run in every bot's process.
pub struct PyTeam {
    python: PathBuf,
    path: PathBuf,
    source: String,
    game: String,
}

impl PyTeam {
    pub fn read(python: &Path, dir: &Path, game: &str) -> Result<Self, BotFailure> {
        let path = dir.join("main.py");
        let source = std::fs::read_to_string(&path).map_err(|e| {
            BotFailure::exception("FileNotFoundError", format!("{}: {e}", path.display()))
        })?;
        Ok(Self {
            python: python.to_path_buf(),
            path,
            source,
            game: game.to_string(),
        })
    }

    pub fn spawn(&self, ctx: &SpawnCtx) -> Result<Box<dyn Bot>, BotFailure> {
        Ok(Box::new(PyBot::spawn(ctx, self)?))
    }
}

pub struct PyBot {
    child: Child,
    stdin: ChildStdin,
    from_bot: Receiver<Message>,
    /// Stopped at its step deadline; its next turn resumes it.
    stopped: bool,
    step_time: Duration,
}

impl PyBot {
    /// Starts the process and, once it is ready, loads `main.py` within the step budget.
    fn spawn(ctx: &SpawnCtx, team: &PyTeam) -> Result<Self, BotFailure> {
        let mut command = Command::new(&team.python);
        command.env_clear();
        for name in BOT_ENV_ALLOWLIST {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command
            .args(["-m", "ucbc._bot"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| BotFailure::Crash(format!("could not start bot process: {e}")))?;
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let (to_runner, from_bot) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message: Message = line.map_err(|e| e.to_string()).and_then(|line| {
                    serde_json::from_str(&line).map_err(|e| format!("bad message from bot: {e}"))
                });
                let readable = message.is_ok();
                if to_runner.send(message).is_err() || !readable {
                    return;
                }
            }
        });
        let mut bot = Self {
            child,
            stdin,
            from_bot,
            stopped: false,
            step_time: ctx.limits.step_time(),
        };
        let init = json!({
            "identity": {
                "bot_id": ctx.bot.id.0,
                "team": ctx.bot.team.0,
                "team_name": ctx.team.name,
                "seed": ctx.seed,
                "game": team.game,
            },
            "source": team.source,
            "path": team.path,
            "memory_bytes": ctx.limits.memory_bytes,
        });
        if !bot.write(&init) {
            return Err(BotFailure::Crash(
                "bot process exited while starting".into(),
            ));
        }
        match bot.from_bot.recv_timeout(STARTUP) {
            Ok(Ok(FromBot::Ready(()))) => {}
            Ok(Ok(_)) => return Err(BotFailure::Crash("bot protocol error".into())),
            Ok(Err(msg)) => return Err(BotFailure::Crash(msg)),
            Err(_) => return Err(BotFailure::Crash("bot process did not start".into())),
        }
        match bot.from_bot.recv_timeout(bot.step_time) {
            Ok(Ok(FromBot::Loaded(None))) => Ok(bot),
            Ok(Ok(FromBot::Loaded(Some(failure)))) => Err(failure.into()),
            Ok(Ok(_)) => Err(BotFailure::Crash("bot protocol error".into())),
            Ok(Err(msg)) => Err(BotFailure::Crash(msg)),
            Err(RecvTimeoutError::Timeout) => Err(BotFailure::exception(
                "TimeoutError",
                format!("main.py took longer than {} ms to load", ctx.limits.step_ms),
            )),
            Err(RecvTimeoutError::Disconnected) => {
                Err(BotFailure::Crash("bot process exited while loading".into()))
            }
        }
    }

    fn write(&mut self, msg: &impl Serialize) -> bool {
        let mut line = serde_json::to_string(msg).expect("messages serialize");
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).is_ok()
    }

    fn signal(&self, signal: Signal) {
        let _ = kill(Pid::from_raw(self.child.id() as i32), signal);
    }

    /// Lets the bot run until it reports `Done` or the budget ends, answering its
    /// queries and actions meanwhile; the time those take is the engine's and extends
    /// the deadline. At the deadline the process is stopped and the step ends with
    /// nothing more than it already did.
    fn wait(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let started = Instant::now();
        loop {
            let deadline = started + self.step_time + ctx.engine_time();
            let left = deadline.saturating_duration_since(Instant::now());
            let message = match self.from_bot.recv_timeout(left) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => {
                    self.signal(Signal::SIGSTOP);
                    self.stopped = true;
                    return StepResult::ok();
                }
                Err(RecvTimeoutError::Disconnected) => return exited(),
            };
            let reply = match message {
                Ok(FromBot::Query(q)) => match ctx.query(&q) {
                    Ok(v) => json!({ "ok": v }),
                    Err(e) => error("query", e),
                },
                Ok(FromBot::Act(a)) => match ctx.act(&a) {
                    Ok(v) => json!({ "ok": v }),
                    Err(e @ ActionError::SetOver) => error("set_over", e),
                    Err(e) => error("action", e),
                },
                Ok(FromBot::Done(reply)) => return reply.into(),
                Ok(FromBot::Ready(()) | FromBot::Loaded(_)) => {
                    return StepResult::failed(BotFailure::Crash("bot protocol error".into()));
                }
                Err(msg) => return StepResult::failed(BotFailure::Crash(msg)),
            };
            if !self.write(&ToBot::Reply(reply)) {
                return exited();
            }
        }
    }
}

impl Bot for PyBot {
    /// A stopped bot picks up where it stopped and this turn ends when that work
    /// returns; otherwise the turn is a fresh step.
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        if self.stopped {
            self.signal(Signal::SIGCONT);
            self.stopped = false;
        } else if !self.write(&ToBot::Step {
            set_index: ctx.set_index,
            tick: ctx.tick,
        }) {
            return exited();
        }
        self.wait(ctx)
    }
}

impl Drop for PyBot {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn exited() -> StepResult {
    StepResult::failed(BotFailure::Crash("bot process exited".into()))
}

fn error(kind: &str, e: impl std::fmt::Display) -> Value {
    json!({ "err": { "kind": kind, "message": e.to_string() } })
}
