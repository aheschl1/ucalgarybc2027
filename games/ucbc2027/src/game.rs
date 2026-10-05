use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, Outcome,
    QueryError, Request, SetSetup, TeamId,
};

use crate::coord::Coord;
use crate::map::{Environment, Map};
use crate::state::{Item, State};
use crate::unit::{Lab, Unit};
use crate::view::{Dropped, ItemView, Snapshot, Spawned, TeamView, UnitEntry, UnitView};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    /// This bot.
    Me(Request<(), UnitView>),
    /// Your team's bones.
    Bones(Request<(), u32>),
    /// Fossils your team has deposited at its lab.
    Fossils(Request<(), u32>),
    /// What is on a tile; None if nothing.
    Item(Request<Coord, Option<ItemView>>),
    /// What a tile is made of.
    Environment(Request<Coord, Environment>),
    /// Tiles across the board.
    Width(Request<(), usize>),
    /// Tiles down the board.
    Height(Request<(), usize>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Does nothing.
    Noop(Request<(), ()>),
    /// Lab only, once per turn, for bones: a level 1 dino on a free tile next to the
    /// lab. It steps from the next tick.
    Spawn(Request<Coord, Spawned>),
    /// Dino only, once per turn: to a free tile within move range. Replies with the
    /// new position.
    Move(Request<Coord, Coord>),
    /// Dino only: pick up the fossil on a tile within action range. A dino holds one
    /// thing at a time. Free, as often as you like.
    Grab(Request<Coord, ItemView>),
    /// Dino only: put down what it holds, on a free tile within action range or on
    /// your own lab to deposit it. Free, as often as you like.
    Drop(Request<Coord, Dropped>),
}

pub struct Ucbc2027 {
    pub(crate) bots: BotManager<Unit>,
    pub(crate) state: State,
    pub(crate) turn: Turn,
    seed: u64,
}

/// What the stepping bot has used of its turn. It belongs to the step, not the bot.
#[derive(Default)]
pub(crate) struct Turn {
    pub spawned: bool,
    pub moved: bool,
}

impl Ucbc2027 {
    pub(crate) fn item_view(&self, item: Item) -> ItemView {
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

    /// The team's highest dino level, 0 with no dinos.
    fn top_level(&self, team: TeamId) -> u32 {
        self.bots
            .ids()
            .into_iter()
            .map(|id| &self.bots[id])
            .filter(|bot| bot.team() == team)
            .filter_map(|bot| bot.as_dino())
            .map(|dino| dino.level)
            .max()
            .unwrap_or(0)
    }
}

/// The teams in `field` with the highest `score`.
fn most(field: Vec<TeamId>, score: impl Fn(TeamId) -> u32) -> Vec<TeamId> {
    let top = field.iter().map(|&team| score(team)).max();
    field
        .into_iter()
        .filter(|&team| Some(score(team)) == top)
        .collect()
}

/// The set's map from `game_config`, `{"maps": ["<a map file in base64>", ...]}`: one map
/// plays every set, otherwise set `i` plays `maps[i]`. Without `maps`, the standard one.
fn map_from(config: Option<&Value>, set_index: u32) -> Result<Map, EngineError> {
    let Some(maps) = config.and_then(|c| c.get("maps")) else {
        return Ok(Map::standard());
    };
    let bad = |why: String| EngineError::Config(format!("game_config.maps: {why}"));
    let maps = maps
        .as_array()
        .ok_or_else(|| bad("expected a list of base64 strings".into()))?;
    let i = if maps.len() == 1 {
        0
    } else {
        set_index as usize
    };
    let map = maps.get(i).ok_or_else(|| {
        bad(format!(
            "set {set_index} has no map; give 1 map or one per set, got {}",
            maps.len()
        ))
    })?;
    let bad = |why: String| EngineError::Config(format!("game_config.maps[{i}]: {why}"));
    let bytes = map
        .as_str()
        .ok_or_else(|| bad("expected a base64 string".into()))
        .and_then(|s| BASE64.decode(s).map_err(|e| bad(e.to_string())))?;
    Map::decode(&bytes).map_err(|e| bad(e.to_string()))
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
        let map = map_from(setup.game_config.as_ref(), setup.set_index)?;
        let mut bots = BotManager::new();
        // In step order, so the first team's lab steps first.
        for &team in &setup.teams {
            bots.spawn(team, Unit::Lab(Lab::new(map.lab(team))));
        }
        Ok(Self {
            bots,
            state: State::new(map),
            turn: Turn::default(),
            seed: setup.seed,
        })
    }

    fn bots(&self) -> &BotManager<Unit> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<Unit> {
        &mut self.bots
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        let off_board = |at: Coord| QueryError::Rejected(format!("{at} is off the board"));
        match query {
            Queries::Me(q) => Ok(q.reply(self.unit_view(bot))),
            Queries::Bones(q) => Ok(q.reply(self.state.team(self.bots[bot].team()).bones)),
            Queries::Fossils(q) => Ok(q.reply(self.state.team(self.bots[bot].team()).fossils)),
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
            Action::Grab(a) => Ok(a.reply(self.grab(bot, *a)?)),
            Action::Drop(a) => Ok(a.reply(self.drop(bot, *a)?)),
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
            team.bones += team.income();
        }
        self.state.tick += 1;
    }

    fn status(&self) -> GameStatus {
        GameStatus::InProgress
    }

    /// Most points wins, then most fossils deposited, then the highest level dino,
    /// then a coin toss.
    fn tick_limit(&self, _max_ticks: u32) -> Outcome {
        let field = self.state.team_ids();
        let field = most(field, |team| self.state.team(team).points());
        if let [winner] = field[..] {
            return Outcome::win(winner).with_detail("most points");
        }
        let field = most(field, |team| self.state.team(team).fossils);
        if let [winner] = field[..] {
            return Outcome::win(winner).with_detail("most fossils");
        }
        let field = most(field, |team| self.top_level(team));
        if let [winner] = field[..] {
            return Outcome::win(winner).with_detail("highest level dino");
        }
        let winner = field[(self.seed % field.len() as u64) as usize];
        Outcome::win(winner).with_detail("coin toss")
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
                .map(|t| TeamView {
                    bones: t.bones,
                    fossils: t.fossils,
                })
                .collect(),
        }
    }
}
