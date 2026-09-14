use std::collections::HashMap;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{ActionError, BotFailure, EngineError, QueryError};
use crate::ids::{BotId, BotRef, TeamId};
use crate::payload::decode;
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

/// One set of one game, in the game's own types. The engine decodes each bot's
/// tagged-JSON payloads into `Query` and `Action` and encodes the responses and
/// snapshots back, so a game never touches JSON.
///
/// A tick is one run of [`schedule`](Self::schedule); a step is one scheduled bot's
/// turn within it. Per step the engine forwards the bot's queries and actions, then
/// calls [`end_step`](Self::end_step) or [`bot_failed`](Self::bot_failed), and drains
/// [`despawned`](Self::despawned). After the last step it calls
/// [`end_tick`](Self::end_tick), unless the set completed during the tick. The game
/// decides which bots exist, which team owns each, and the step order. A set ends
/// when the game reports one team left, a draw, or the tick limit is reached.
pub trait Game: Send + 'static {
    /// Registry key, e.g. `"tictactoe"`.
    const NAME: &'static str;
    /// A `#[serde(tag = "type")]` enum of what bots may ask.
    type Query: DeserializeOwned;
    type QueryResponse: Serialize;
    /// A `#[serde(tag = "type")]` enum of what bots may do.
    type Action: DeserializeOwned;
    type ActionResponse: Serialize;
    type Snapshot: Serialize;

    fn create(setup: &SetSetup) -> Result<Self, EngineError>
    where
        Self: Sized;

    /// Live bots to step this tick, in order. A dead or duplicated bot is a game bug.
    fn schedule(&mut self) -> Vec<BotRef>;

    fn handle_query(
        &self,
        bot: BotRef,
        query: Self::Query,
    ) -> Result<Self::QueryResponse, QueryError>;

    /// Only the stepping bot ever calls this. On `Err` the state is untouched and the
    /// bot may try again. `Ok` carries what the action produced for the bot to see.
    fn apply_action(
        &mut self,
        bot: BotRef,
        action: Self::Action,
    ) -> Result<Self::ActionResponse, ActionError>;

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
    fn snapshot(&self) -> Self::Snapshot;
}

/// [`Game`] over JSON payloads: the calling convention between engine and bots.
/// Implemented for every `Game`; the engine only ever holds `Box<dyn DynGame>`.
pub trait DynGame: Send {
    fn schedule(&mut self) -> Vec<BotRef>;
    fn handle_query(&self, bot: BotRef, query: &Value) -> Result<Value, QueryError>;
    fn apply_action(&mut self, bot: BotRef, action: &Value) -> Result<Value, ActionError>;
    fn end_step(&mut self, bot: BotRef);
    fn bot_failed(&mut self, bot: BotRef, failure: &BotFailure);
    fn end_tick(&mut self);
    fn despawned(&mut self) -> Vec<BotId>;
    fn status(&self) -> GameStatus;
    fn snapshot(&self) -> Value;
}

impl<G: Game> DynGame for G {
    fn schedule(&mut self) -> Vec<BotRef> {
        Game::schedule(self)
    }

    fn handle_query(&self, bot: BotRef, query: &Value) -> Result<Value, QueryError> {
        let response = Game::handle_query(self, bot, decode(query)?)?;
        Ok(encode(response))
    }

    fn apply_action(&mut self, bot: BotRef, action: &Value) -> Result<Value, ActionError> {
        let response = Game::apply_action(self, bot, decode(action)?)?;
        Ok(encode(response))
    }

    fn end_step(&mut self, bot: BotRef) {
        Game::end_step(self, bot)
    }

    fn bot_failed(&mut self, bot: BotRef, failure: &BotFailure) {
        Game::bot_failed(self, bot, failure)
    }

    fn end_tick(&mut self) {
        Game::end_tick(self)
    }

    fn despawned(&mut self) -> Vec<BotId> {
        Game::despawned(self)
    }

    fn status(&self) -> GameStatus {
        Game::status(self)
    }

    fn snapshot(&self) -> Value {
        encode(Game::snapshot(self))
    }
}

/// Plain data types always serialize; a failure here is a bug in the game's types.
fn encode<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("game response serializes")
}

/// Creates a set of a registered game.
pub type GameFactory =
    Box<dyn Fn(&SetSetup) -> Result<Box<dyn DynGame>, EngineError> + Send + Sync>;

/// Games known to this engine build, by [`Game::NAME`].
#[derive(Default)]
pub struct GameRegistry {
    factories: HashMap<&'static str, GameFactory>,
}

impl GameRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<G: Game>(&mut self) -> &mut Self {
        let factory: GameFactory =
            Box::new(|setup| G::create(setup).map(|g| Box::new(g) as Box<dyn DynGame>));
        self.factories.insert(G::NAME, factory);
        self
    }

    pub fn get(&self, name: &str) -> Result<&GameFactory, EngineError> {
        self.factories
            .get(name)
            .ok_or_else(|| EngineError::UnknownGame(name.to_string()))
    }
}
