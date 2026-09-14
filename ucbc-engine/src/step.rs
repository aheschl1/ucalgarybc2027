use serde_json::Value;

use crate::error::{ActionError, BotFailure, QueryError};
use crate::game::{Game, GameStatus};
use crate::ids::{BotRef, TeamInfo};

/// A bot's handle on the game during its own step.
pub struct StepCtx<'a> {
    pub bot: BotRef,
    pub team: &'a TeamInfo,
    pub set_index: u32,
    pub tick: u32,
    /// Per bot, per set; stable for a given match seed.
    pub seed: u64,
    game: &'a mut dyn Game,
    accepted: &'a mut Vec<Value>,
}

impl<'a> StepCtx<'a> {
    pub(crate) fn new(
        bot: BotRef,
        team: &'a TeamInfo,
        set_index: u32,
        tick: u32,
        seed: u64,
        game: &'a mut dyn Game,
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
        }
    }

    pub fn status(&self) -> GameStatus {
        self.game.status()
    }

    pub fn query(&self, query: &Value) -> Result<Value, QueryError> {
        self.game.handle_query(self.bot, query)
    }

    /// A rejection is an error the bot can handle; the state is unchanged. Once the
    /// set is complete every action is `SetOver`.
    pub fn act(&mut self, action: &Value) -> Result<Value, ActionError> {
        if matches!(self.game.status(), GameStatus::Complete(_)) {
            return Err(ActionError::SetOver);
        }
        let response = self.game.apply_action(self.bot, action)?;
        self.accepted.push(action.clone());
        Ok(response)
    }
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub outcome: Result<(), BotFailure>,
    /// Captured output of the step.
    pub stdout: String,
}

impl StepResult {
    pub fn ok() -> Self {
        Self {
            outcome: Ok(()),
            stdout: String::new(),
        }
    }

    pub fn failed(failure: BotFailure) -> Self {
        Self {
            outcome: Err(failure),
            stdout: String::new(),
        }
    }

    pub fn with_stdout(mut self, stdout: impl Into<String>) -> Self {
        self.stdout = stdout.into();
        self
    }
}
