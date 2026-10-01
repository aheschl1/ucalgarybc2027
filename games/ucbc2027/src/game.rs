use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotRef, EngineError, Game, GameStatus, QueryError, Request,
    SetSetup, TeamId,
};

const WIDTH: usize = 16;
const HEIGHT: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Environment {
    Empty,
    Wall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Item {
    Player { team: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Tile {
    pub environment: Environment,
    pub item: Option<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct State {
    pub tick: u32,
    pub board: Vec<Vec<Tile>>,
}

impl State {
    /// An empty board, team 0's player top-left and team 1's bottom-right.
    fn new() -> Self {
        let empty = Tile {
            environment: Environment::Empty,
            item: None,
        };
        let mut board = vec![vec![empty; WIDTH]; HEIGHT];
        board[0][0].item = Some(Item::Player { team: 0 });
        board[HEIGHT - 1][WIDTH - 1].item = Some(Item::Player { team: 1 });
        Self { tick: 0, board }
    }

    fn tile(&self, at: &At) -> Result<&Tile, QueryError> {
        self.board
            .get(at.y)
            .and_then(|row| row.get(at.x))
            .ok_or_else(|| QueryError::Rejected(format!("({}, {}) is off the board", at.x, at.y)))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct At {
    pub x: usize,
    pub y: usize,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    Item(Request<At, Option<Item>>),
    Environment(Request<At, Environment>),
    Width(Request<(), usize>),
    Height(Request<(), usize>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Noop(Request<(), ()>),
}

pub struct Ucbc2027 {
    /// Step order of the bots still in the set.
    order: Vec<TeamId>,
    state: State,
}

impl Game for Ucbc2027 {
    const NAME: &'static str = "ucbc2027";
    type Query = Queries;
    type Action = Action;
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
            state: State::new(),
        })
    }

    fn schedule(&mut self) -> Vec<BotRef> {
        self.order
            .iter()
            .map(|t| BotRef::new(u64::from(t.0), t.0))
            .collect()
    }

    fn handle_query(&self, _bot: BotRef, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::Item(q) => Ok(q.reply(self.state.tile(&q)?.item)),
            Queries::Environment(q) => Ok(q.reply(self.state.tile(&q)?.environment)),
            Queries::Width(q) => Ok(q.reply(WIDTH)),
            Queries::Height(q) => Ok(q.reply(HEIGHT)),
        }
    }

    fn apply_action(&mut self, _bot: BotRef, action: Action) -> Result<Answer, ActionError> {
        match action {
            Action::Noop(a) => Ok(a.reply(())),
        }
    }

    fn end_step(&mut self, _bot: BotRef) {}

    fn bot_failed(&mut self, bot: BotRef, _failure: &BotFailure) {
        self.order.retain(|t| *t != bot.team);
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
