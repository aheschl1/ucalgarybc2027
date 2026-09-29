use std::collections::HashMap;

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{ActionError, BotFailure, EngineError, QueryError};
use crate::ids::{BotId, BotRef, TeamId};
use crate::payload::{decode, encode};
use crate::query::Answer;
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
/// when the game reports a completed status, or at the tick limit with the outcome
/// from [`tick_limit`](Self::tick_limit).
pub trait Game: Send + 'static {
    /// Registry key, e.g. `"tictactoe"`.
    const NAME: &'static str;
    /// A `#[serde(tag = "type")]` enum of what bots may ask, each variant holding a
    /// [`Query`](crate::Query) that names its reply type.
    type Query: DeserializeOwned + JsonSchema;
    /// A `#[serde(tag = "type")]` enum of what bots may do.
    type Action: DeserializeOwned + JsonSchema;
    type ActionResponse: Serialize + JsonSchema;
    type Snapshot: Serialize + JsonSchema;

    fn create(setup: &SetSetup) -> Result<Self, EngineError>
    where
        Self: Sized;

    /// Live bots to step this tick, in order. A dead or duplicated bot is a game bug.
    fn schedule(&mut self) -> Vec<BotRef>;

    fn handle_query(&self, bot: BotRef, query: Self::Query) -> Result<Answer, QueryError>;

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

    /// The outcome of a set still in progress after its last tick.
    fn tick_limit(&self, max_ticks: u32) -> Outcome {
        Outcome::draw(format!("tick limit of {max_ticks} reached"))
    }

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
    fn tick_limit(&self, max_ticks: u32) -> Outcome;
    fn snapshot(&self) -> Value;
}

impl<G: Game> DynGame for G {
    fn schedule(&mut self) -> Vec<BotRef> {
        Game::schedule(self)
    }

    fn handle_query(&self, bot: BotRef, query: &Value) -> Result<Value, QueryError> {
        Ok(Game::handle_query(self, bot, decode(query)?)?.into_value())
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

    fn tick_limit(&self, max_ticks: u32) -> Outcome {
        Game::tick_limit(self, max_ticks)
    }

    fn snapshot(&self) -> Value {
        encode(Game::snapshot(self))
    }
}

/// Creates a set of a registered game.
pub type GameFactory =
    Box<dyn Fn(&SetSetup) -> Result<Box<dyn DynGame>, EngineError> + Send + Sync>;

/// What a game exposes to bots, as JSON Schema: the input to SDK generation.
#[derive(Clone, Debug, Serialize)]
pub struct GameApi {
    pub name: &'static str,
    /// The game's Rust type name, e.g. `TicTacToe`.
    pub type_name: &'static str,
    pub query: Value,
    pub action: Value,
    pub action_response: Value,
    pub snapshot: Value,
}

impl GameApi {
    fn of<G: Game>() -> Self {
        Self {
            name: G::NAME,
            type_name: std::any::type_name::<G>()
                .rsplit("::")
                .next()
                .unwrap_or("Game"),
            query: schema_for!(G::Query).to_value(),
            action: schema_for!(G::Action).to_value(),
            action_response: schema_for!(G::ActionResponse).to_value(),
            snapshot: schema_for!(G::Snapshot).to_value(),
        }
    }
}

struct Entry {
    factory: GameFactory,
    api: fn() -> GameApi,
}

/// Games known to this engine build, by [`Game::NAME`].
#[derive(Default)]
pub struct GameRegistry {
    games: HashMap<&'static str, Entry>,
}

impl GameRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<G: Game>(&mut self) -> &mut Self {
        let factory: GameFactory =
            Box::new(|setup| G::create(setup).map(|g| Box::new(g) as Box<dyn DynGame>));
        let entry = Entry {
            factory,
            api: GameApi::of::<G>,
        };
        self.games.insert(G::NAME, entry);
        self
    }

    /// Registered game names, sorted.
    pub fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.games.keys().copied().collect();
        names.sort_unstable();
        names
    }

    fn entry(&self, name: &str) -> Result<&Entry, EngineError> {
        self.games
            .get(name)
            .ok_or_else(|| EngineError::UnknownGame(name.to_string()))
    }

    pub fn get(&self, name: &str) -> Result<&GameFactory, EngineError> {
        self.entry(name).map(|e| &e.factory)
    }

    pub fn api(&self, name: &str) -> Result<GameApi, EngineError> {
        self.entry(name).map(|e| (e.api)())
    }
}
