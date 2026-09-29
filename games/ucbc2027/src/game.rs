use std::{println, unimplemented};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotRef, EngineError, Game, GameStatus, Query, QueryError,
    SetSetup, TeamId,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Environment {
    Empty,
    Wall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Item {
    Player,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Tile {
    environment: Environment,
    item: Option<Item>,
}

/// How many ticks have ended this set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct State {
    pub board: Vec<Vec<Tile>>,
}

impl State {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub fn blank(width: usize, height: usize) -> Self {
        let board = vec![
            vec![
                Tile {
                    environment: Environment::Empty,
                    item: None
                };
                width
            ];
            height
        ];
        Self { board }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Noop,
}

#[derive(Deserialize, JsonSchema)]
pub struct At {
    pub x: usize,
    pub y: usize,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    Item(Query<At, Option<Item>>),
    Environment(Query<At, Environment>),
    Width(Query<(), usize>),
    Height(Query<(), usize>),
}

struct PlayerBase(TeamId);

pub struct Ucbc2027 {
    state: State,
    tick: u64,
    bots: [PlayerBase; 2],
}

impl Ucbc2027 {
    pub fn new() -> Self {
        Self {
            state: State::blank(100, 100),
            tick: 0,
            bases: [PlayerBase(TeamId(0)), PlayerBase(TeamId(1))],
        }
    }
}

impl Game for Ucbc2027 {
    const NAME: &'static str = "ucbc2027";
    type Query = Queries;
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
        println!("Creating game with setup: {:?}", setup);
        // let first = setup.first_team;
        Ok(Self::new())
    }

    fn schedule(&mut self) -> Vec<BotRef> {
        let i = self.tick as usize % 2;
        let team = self.bases[i].0;
        vec![BotRef::new(0, 0)]
    }

    fn handle_query(&self, _bot: BotRef, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::Item(q) => {
                let At { x, y } = *q;
                if x >= self.state.board.len() || y >= self.state.board[0].len() {
                    return Err(QueryError::Rejected(format!(
                        "Coordinates out of bounds: ({}, {})",
                        x, y
                    )));
                }
                Ok(q.reply(self.state.board[y][x].item.clone()))
            }
            Queries::Environment(q) => {
                let At { x, y } = *q;
                if x >= self.state.board.len() || y >= self.state.board[0].len() {
                    return Err(QueryError::Rejected(format!(
                        "Coordinates out of bounds: ({}, {})",
                        x, y
                    )));
                }
                Ok(q.reply(self.state.board[y][x].environment.clone()))
            }
            Queries::Width(q) => Ok(q.reply(self.state.board[0].len())),
            Queries::Height(q) => Ok(q.reply(self.state.board.len())),
        }
    }

    fn apply_action(&mut self, _bot: BotRef, action: Action) -> Result<(), ActionError> {
        match action {
            Action::Noop => Ok(()),
        }
    }

    fn end_step(&mut self, _bot: BotRef) {}

    fn bot_failed(&mut self, bot: BotRef, _failure: &BotFailure) {}

    fn end_tick(&mut self) {}

    fn status(&self) -> GameStatus {
        GameStatus::InProgress
    }

    fn snapshot(&self) -> State {
        self.state.clone()
    }
}
