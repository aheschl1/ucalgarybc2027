use std::collections::HashMap;

use serde_json::Value;

use crate::error::{ActionError, BotFailure, EngineError, QueryError};
use crate::ids::{BotId, BotRef, TeamId};
use crate::replay::Reason;

#[derive(Clone, Debug)]
pub struct SetSetup {
    pub set_index: u32,
    pub teams: u32,
    /// Rotates each set.
    pub first_team: TeamId,
    pub seed: u64,
    pub game_config: Option<Value>,
}

impl SetSetup {
    pub fn new(
        set_index: u32,
        teams: u32,
        first_team: TeamId,
        seed: u64,
        game_config: Option<Value>,
    ) -> Self {
        Self {
            set_index,
            teams,
            first_team,
            seed,
            game_config,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// `None` is a draw.
    pub winner: Option<TeamId>,
    pub reason: Reason,
    pub detail: String,
}

impl Outcome {
    pub fn win(team: TeamId) -> Self {
        Self {
            winner: Some(team),
            reason: Reason::Win,
            detail: String::new(),
        }
    }

    pub fn draw(detail: impl Into<String>) -> Self {
        Self {
            winner: None,
            reason: Reason::Draw,
            detail: detail.into(),
        }
    }

    /// The last team standing after the others forfeited.
    pub fn forfeit(winner: TeamId, detail: impl Into<String>) -> Self {
        Self {
            winner: Some(winner),
            reason: Reason::Forfeit,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameStatus {
    InProgress,
    Complete(Outcome),
}

/// One set of one game. The engine is the only caller.
///
/// A tick is one run of [`schedule`](Self::schedule); a step is one scheduled bot's
/// turn within it. Per step the engine forwards the bot's queries and actions, then
/// calls [`end_step`](Self::end_step) or [`bot_failed`](Self::bot_failed), and drains
/// [`despawned`](Self::despawned). After the last step it calls
/// [`end_tick`](Self::end_tick), unless the set completed during the tick. The game
/// decides which bots exist, which team owns each, and the step order. A set ends
/// when the game reports one team left, a draw, or the tick limit is reached.
pub trait Game: Send {
    /// Live bots to step this tick, in order. A dead or duplicated bot is a game bug.
    fn schedule(&mut self) -> Vec<BotRef>;

    fn handle_query(&self, bot: BotRef, query: &Value) -> Result<Value, QueryError>;

    /// Only the stepping bot ever calls this. On `Err` the state is untouched and the
    /// bot may try again. `Ok` carries what the action produced for the bot to see.
    fn apply_action(&mut self, bot: BotRef, action: &Value) -> Result<Value, ActionError>;

    /// The bot's step returned normally.
    fn end_step(&mut self, bot: BotRef);

    /// The bot's runtime failed and has been dropped. Its actions so far stand.
    fn bot_failed(&mut self, bot: BotRef, failure: &BotFailure);

    fn end_tick(&mut self);

    /// Bots whose runtimes the engine should release. Drained after every step.
    fn despawned(&mut self) -> Vec<BotId> {
        Vec::new()
    }

    fn status(&self) -> GameStatus;

    /// Full state, recorded once per tick.
    fn snapshot(&self) -> Value;
}

pub trait GameFactory: Send + Sync {
    fn name(&self) -> &'static str;
    fn create(&self, setup: &SetSetup) -> Result<Box<dyn Game>, EngineError>;
}

#[derive(Default)]
pub struct GameRegistry {
    factories: HashMap<&'static str, Box<dyn GameFactory>>,
}

impl GameRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, factory: Box<dyn GameFactory>) -> &mut Self {
        self.factories.insert(factory.name(), factory);
        self
    }

    pub fn get(&self, name: &str) -> Result<&dyn GameFactory, EngineError> {
        self.factories
            .get(name)
            .map(|f| f.as_ref())
            .ok_or_else(|| EngineError::UnknownGame(name.to_string()))
    }
}
