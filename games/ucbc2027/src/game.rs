use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, QueryError,
    Request, SetSetup,
};

use crate::{
    BotType,
    state::{Board, Environment, Item, State},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

impl Coord {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// The up to eight neighbours, skipping any that would go below zero. No upper bound check.
    pub fn around(&self) -> impl Iterator<Item = Coord> {
        let Coord { x, y } = *self;
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&d| d != (0, 0))
            .filter_map(move |(dx, dy)| {
                Some(Coord::new(
                    x.checked_add_signed(dx)?,
                    y.checked_add_signed(dy)?,
                ))
            })
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SpawnRequest {
    pub x: usize,
    pub y: usize,
    pub bot_type: BotType,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    Item(Request<Coord, Option<Item>>),
    Environment(Request<Coord, Environment>),
    Width(Request<(), usize>),
    Height(Request<(), usize>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Noop(Request<(), ()>),
    Spawn(Request<SpawnRequest, ()>),
}

pub struct Ucbc2027 {
    bots: BotManager<BotType>,
    board: Board,
}

impl Game for Ucbc2027 {
    const NAME: &'static str = "ucbc2027";
    type Query = Queries;
    type Action = Action;
    type Snapshot = State;
    type Bot = BotType;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        if setup.teams.len() != 2 {
            return Err(EngineError::Config(format!(
                "ucbc2027 needs exactly 2 teams, got {}",
                setup.teams.len()
            )));
        }
        let mut bots = BotManager::new();
        let mut board = Board::new();
        for &team in &setup.teams {
            let base = bots.schedule(team, BotType::Base);
            board.place(&board.base_location(team), base);
        }
        Ok(Self { bots, board })
    }

    fn bots(&self) -> &BotManager<BotType> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<BotType> {
        &mut self.bots
    }

    fn handle_query(&self, _bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::Item(q) => Ok(q.reply(self.board.tile(&q, &self.bots)?.item)),
            Queries::Environment(q) => Ok(q.reply(self.board.tile(&q, &self.bots)?.environment)),
            Queries::Width(q) => Ok(q.reply(self.board.width())),
            Queries::Height(q) => Ok(q.reply(self.board.height())),
        }
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        match action {
            Action::Noop(a) => Ok(a.reply(())),
            Action::Spawn(req) => {
                let spawner = &self.bots[bot];
                if **spawner != BotType::Base {
                    return Err(ActionError::Invalid("Cannot spawn from this bot.".into()));
                }
                if req.bot_type == BotType::Base {
                    return Err(ActionError::Invalid("A base spawns dinos only.".into()));
                }
                let team = spawner.team();
                let at = Coord::new(req.x, req.y);
                self.board.check_spawn(&at, team)?;
                let spawned = self.bots.schedule(team, req.bot_type);
                self.board.place(&at, spawned);
                Ok(req.reply(()))
            }
        }
    }

    fn end_step(&mut self, _bot: BotId) {}

    fn bot_failed(&mut self, bot: BotId, _failure: &BotFailure) {
        self.board.remove(bot);
    }

    fn end_tick(&mut self) {
        self.board.tick += 1;
    }

    fn status(&self) -> GameStatus {
        GameStatus::InProgress
    }

    fn snapshot(&self) -> State {
        self.board.snapshot(&self.bots)
    }
}
