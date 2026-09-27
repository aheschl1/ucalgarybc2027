use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{
    ActionError, BotFailure, BotRef, EngineError, Game, GameStatus, QueryError, SetSetup, TeamId,
};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Query {
    /// The game state.
    State,
}

/// How many ticks have ended this set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct State {
    pub tick: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Does nothing.
    Noop,
}

/// Two teams, one bot each (bot id equals team id), both stepping every tick, the set's
/// first team first. A bot that fails sits out the rest of the set. Nothing ends a set
/// early, so every set runs to the tick limit and draws.
pub struct Ucbc2027 {
    /// Step order of the bots still in the set.
    order: Vec<TeamId>,
    state: State,
}

impl Game for Ucbc2027 {
    const NAME: &'static str = "ucbc2027";
    type Query = Query;
    type QueryResponse = State;
    type Action = Action;
    type ActionResponse = ();
    type Snapshot = State;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        if setup.teams != 2 {
            return Err(EngineError::Config(format!(
                "ucbc2027 needs exactly 2 teams, got {}",
                setup.teams
            )));
        }
        let first = setup.first_team;
        Ok(Self {
            order: vec![first, TeamId(1 - first.0)],
            state: State { tick: 0 },
        })
    }

    fn schedule(&mut self) -> Vec<BotRef> {
        self.order
            .iter()
            .map(|t| BotRef::new(u64::from(t.0), t.0))
            .collect()
    }

    fn handle_query(&self, _bot: BotRef, query: Query) -> Result<State, QueryError> {
        match query {
            Query::State => Ok(self.state.clone()),
        }
    }

    fn apply_action(&mut self, _bot: BotRef, action: Action) -> Result<(), ActionError> {
        match action {
            Action::Noop => Ok(()),
        }
    }

    fn end_step(&mut self, _bot: BotRef) {}

    fn bot_failed(&mut self, bot: BotRef, _failure: &BotFailure) {
        self.order.retain(|&t| t != bot.team);
    }

    fn end_tick(&mut self) {
        self.state.tick += 1;
    }

    fn status(&self) -> GameStatus {
        GameStatus::InProgress
    }

    fn snapshot(&self) -> State {
        self.state.clone()
    }
}
