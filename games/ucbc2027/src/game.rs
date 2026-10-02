use schemars::JsonSchema;
use serde::Deserialize;
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, QueryError,
    Request, SetSetup,
};

use crate::coord::Coord;
use crate::map::{Environment, Map};
use crate::rules;
use crate::state::{Item, State};
use crate::unit::{Lab, Unit};
use crate::view::{ItemView, Snapshot, Spawned, TeamView, UnitEntry, UnitView};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    /// This bot.
    Me(Request<(), UnitView>),
    /// Your team's bones.
    Bones(Request<(), u32>),
    Item(Request<Coord, Option<ItemView>>),
    Environment(Request<Coord, Environment>),
    Width(Request<(), usize>),
    Height(Request<(), usize>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Noop(Request<(), ()>),
    /// Lab only, once per turn, for bones: a level 1 dino on a free tile next to the
    /// lab. It steps from the next tick.
    Spawn(Request<Coord, Spawned>),
    /// Dino only, once per turn: to a free tile within move range. Replies with the
    /// new position.
    Move(Request<Coord, Coord>),
}

pub struct Ucbc2027 {
    pub(crate) bots: BotManager<Unit>,
    pub(crate) state: State,
    pub(crate) turn: Turn,
}

/// What the stepping bot has used of its turn. It belongs to the step, not the bot.
#[derive(Default)]
pub(crate) struct Turn {
    pub spawned: bool,
    pub moved: bool,
}

impl Ucbc2027 {
    fn item_view(&self, item: Item) -> ItemView {
        match item {
            Item::Dino(id) => {
                let bot = &self.bots[id];
                let Unit::Dino(dino) = &**bot else {
                    unreachable!("bot {id} on the board is not a dino")
                };
                ItemView::Dino {
                    team: bot.team().0,
                    level: dino.level,
                }
            }
            Item::Fossil => ItemView::Fossil,
        }
    }

    fn unit_view(&self, bot: BotId) -> UnitView {
        match &*self.bots[bot] {
            Unit::Lab(lab) => UnitView::Lab {
                origin: lab.origin,
                health: lab.health,
            },
            Unit::Dino(dino) => UnitView::Dino {
                pos: dino.pos,
                level: dino.level,
                health: dino.health,
                held: dino.held.map(|item| self.item_view(item)),
            },
        }
    }
}

impl Game for Ucbc2027 {
    const NAME: &'static str = "ucbc2027";
    type Query = Queries;
    type Action = Action;
    type Snapshot = Snapshot;
    type Bot = Unit;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        if setup.teams.len() != 2 {
            return Err(EngineError::Config(format!(
                "ucbc2027 needs exactly 2 teams, got {}",
                setup.teams.len()
            )));
        }
        let map = Map::standard();
        let mut bots = BotManager::new();
        // In step order, so the first team's lab steps first.
        for &team in &setup.teams {
            bots.spawn(team, Unit::Lab(Lab::new(map.lab(team))));
        }
        Ok(Self {
            bots,
            state: State::new(map),
            turn: Turn::default(),
        })
    }

    fn bots(&self) -> &BotManager<Unit> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<Unit> {
        &mut self.bots
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        let off_board =
            |at: Coord| QueryError::Rejected(format!("({}, {}) is off the board", at.x, at.y));
        match query {
            Queries::Me(q) => Ok(q.reply(self.unit_view(bot))),
            Queries::Bones(q) => Ok(q.reply(self.state.team(self.bots[bot].team()).bones)),
            Queries::Item(q) => {
                let item = self.state.items.get(*q).ok_or_else(|| off_board(*q))?;
                Ok(q.reply(item.map(|item| self.item_view(item))))
            }
            Queries::Environment(q) => {
                let env = self.state.map.env(*q).ok_or_else(|| off_board(*q))?;
                Ok(q.reply(env))
            }
            Queries::Width(q) => Ok(q.reply(self.state.map.width())),
            Queries::Height(q) => Ok(q.reply(self.state.map.height())),
        }
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        match action {
            Action::Noop(a) => Ok(a.reply(())),
            Action::Spawn(a) => Ok(a.reply(self.spawn(bot, *a)?)),
            Action::Move(a) => Ok(a.reply(self.move_to(bot, *a)?)),
        }
    }

    fn end_step(&mut self, _bot: BotId) {
        self.turn = Turn::default();
    }

    fn bot_failed(&mut self, bot: BotId, _failure: &BotFailure) {
        self.turn = Turn::default();
        if let Unit::Dino(dino) = &*self.bots[bot] {
            self.state.clear_dino(dino);
        }
    }

    fn end_tick(&mut self) {
        for team in &mut self.state.teams {
            team.bones += rules::BASE_INCOME;
        }
        self.state.tick += 1;
    }

    fn status(&self) -> GameStatus {
        GameStatus::InProgress
    }

    fn snapshot(&self) -> Snapshot {
        let state = &self.state;
        Snapshot {
            tick: state.tick,
            environment: state.map.rows().map(<[_]>::to_vec).collect(),
            units: self
                .bots
                .ids()
                .into_iter()
                .map(|id| UnitEntry {
                    id: id.0,
                    team: self.bots[id].team().0,
                    unit: self.unit_view(id),
                })
                .collect(),
            fossils: state
                .items
                .iter()
                .filter(|&(_, item)| *item == Some(Item::Fossil))
                .map(|(at, _)| at)
                .collect(),
            teams: state
                .teams
                .iter()
                .map(|t| TeamView { bones: t.bones })
                .collect(),
        }
    }
}
