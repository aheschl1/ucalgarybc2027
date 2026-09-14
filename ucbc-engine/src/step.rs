use std::time::{Duration, Instant};

use serde_json::Value;

use crate::error::{ActionError, BotFailure, QueryError};
use crate::game::{DynGame, GameStatus};
use crate::ids::{BotRef, TeamInfo};

/// A bot's handle on the game during its own step.
pub struct StepCtx<'a> {
    pub bot: BotRef,
    pub team: &'a TeamInfo,
    pub set_index: u32,
    pub tick: u32,
    /// Per bot, per set; stable for a given match seed.
    pub seed: u64,
    game: &'a mut dyn DynGame,
    accepted: &'a mut Vec<Value>,
    engine_time: Duration,
}

impl<'a> StepCtx<'a> {
    pub(crate) fn new(
        bot: BotRef,
        team: &'a TeamInfo,
        set_index: u32,
        tick: u32,
        seed: u64,
        game: &'a mut dyn DynGame,
        accepted: &'a mut Vec<Value>,
    ) -> Self {
        Self {
            bot,
            team,
            set_index,
            tick,
            seed,
            game,
            accepted,
            engine_time: Duration::ZERO,
        }
    }

    /// Time the engine has spent answering this step's queries and actions. Not the
    /// bot's, so its budget and its recorded time exclude it.
    pub fn engine_time(&self) -> Duration {
        self.engine_time
    }

    fn timed<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let started = Instant::now();
        let out = f(self);
        self.engine_time += started.elapsed();
        out
    }

    pub fn status(&self) -> GameStatus {
        self.game.status()
    }

    pub fn query(&mut self, query: &Value) -> Result<Value, QueryError> {
        self.timed(|ctx| ctx.game.handle_query(ctx.bot, query))
    }

    /// A rejection is an error the bot can handle; the state is unchanged. Once the
    /// set is complete every action is `SetOver`.
    pub fn act(&mut self, action: &Value) -> Result<Value, ActionError> {
        self.timed(|ctx| {
            if matches!(ctx.game.status(), GameStatus::Complete(_)) {
                return Err(ActionError::SetOver);
            }
            let response = ctx.game.apply_action(ctx.bot, action)?;
            ctx.accepted.push(action.clone());
            Ok(response)
        })
    }
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub outcome: Result<(), BotFailure>,
    /// Captured output of the step.
    pub stdout: String,
    /// Bytes the bot's runtime holds after the step, if it can tell.
    pub memory: Option<u64>,
}

impl StepResult {
    pub fn ok() -> Self {
        Self {
            outcome: Ok(()),
            stdout: String::new(),
            memory: None,
        }
    }

    pub fn failed(failure: BotFailure) -> Self {
        Self {
            outcome: Err(failure),
            stdout: String::new(),
            memory: None,
        }
    }

    pub fn with_stdout(mut self, stdout: impl Into<String>) -> Self {
        self.stdout = stdout.into();
        self
    }

    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.memory = Some(bytes);
        self
    }
}
