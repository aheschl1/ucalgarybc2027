use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use schemars::JsonSchema;
use serde::Deserialize;
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, Outcome,
    QueryError, Request, SetSetup, TeamId,
};

use crate::config::Config;
use crate::map::{Coord, Dir, Map, Tile};
use crate::view::{Kind, MapView, Observation, Snapshot, UnitView};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    /// What you see: tiles and units within your vision.
    Observe(Request<(), Observation>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move one tile onto walkable, unoccupied ground; returns where you are.
    Move(Request<Heading, Coord>),
    /// Commander only, next to your base: a worker on a free tile next to the base.
    /// Returns its id.
    Spawn(Request<(), u64>),
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Heading {
    pub dir: Dir,
}

pub struct Unit {
    kind: Kind,
    pos: Coord,
}

pub struct Frontier {
    config: Config,
    bots: BotManager<Unit>,
    map: Map,
    /// Bot id + 1 standing on each tile, 0 if none.
    occupant: Vec<u64>,
    /// Indexed by team id.
    bases: [Coord; 2],
    commanders: [BotId; 2],
    /// Ticks begun; the snapshot after them is numbered this.
    ticks: u32,
    /// What the stepping bot has done this step.
    moved: bool,
    spawned: bool,
    /// Units changed and removed since the last snapshot.
    changed: BTreeSet<BotId>,
    removed: Vec<u64>,
    outcome: Option<Outcome>,
}

fn invalid(message: &str) -> ActionError {
    ActionError::Invalid(message.into())
}

fn adjacent(a: Coord, b: Coord) -> bool {
    a != b && a.x.abs_diff(b.x) <= 1 && a.y.abs_diff(b.y) <= 1
}

impl Frontier {
    fn idx(&self, at: Coord) -> usize {
        at.y * self.map.size() + at.x
    }

    fn free(&self, at: Coord) -> bool {
        self.map.tile(at).walkable() && self.occupant[self.idx(at)] == 0
    }

    fn view(&self, id: BotId) -> UnitView {
        let unit = &self.bots[id];
        UnitView {
            id: id.0,
            team: unit.team().0,
            kind: unit.kind,
            pos: unit.pos,
        }
    }

    fn place(&mut self, team: TeamId, kind: Kind, at: Coord) -> BotId {
        let id = self.bots.spawn(team, Unit { kind, pos: at });
        let i = self.idx(at);
        self.occupant[i] = id.0 + 1;
        self.changed.insert(id);
        id
    }

    fn move_unit(&mut self, bot: BotId, dir: Dir) -> Result<Coord, ActionError> {
        if self.moved {
            return Err(invalid("already moved this step"));
        }
        let from = self.bots[bot].pos;
        let to = self
            .map
            .step(from, dir)
            .ok_or_else(|| invalid("off the map"))?;
        if !self.free(to) {
            return Err(invalid("blocked"));
        }
        let (a, b) = (self.idx(from), self.idx(to));
        self.occupant.swap(a, b);
        self.bots[bot].pos = to;
        self.moved = true;
        self.changed.insert(bot);
        Ok(to)
    }

    fn spawn(&mut self, bot: BotId) -> Result<u64, ActionError> {
        let team = self.bots[bot].team();
        let base = self.bases[team.0 as usize];
        if self.bots[bot].kind != Kind::Commander {
            return Err(invalid("only a commander can spawn"));
        }
        if self.spawned {
            return Err(invalid("already spawned this step"));
        }
        if !adjacent(self.bots[bot].pos, base) {
            return Err(invalid("the commander must stand next to the base"));
        }
        let ids = self.bots.ids();
        if ids
            .iter()
            .filter(|&&id| self.bots[id].team() == team)
            .count() as u32
            >= self.config.units
        {
            return Err(invalid("the team is at its unit cap"));
        }
        let size = self.map.size();
        let at = (base.y.saturating_sub(1)..=(base.y + 1).min(size - 1))
            .flat_map(|y| {
                (base.x.saturating_sub(1)..=(base.x + 1).min(size - 1))
                    .map(move |x| Coord::new(x, y))
            })
            .find(|&at| self.free(at))
            .ok_or_else(|| invalid("no free tile next to the base"))?;
        self.spawned = true;
        Ok(self.place(team, Kind::Worker, at).0)
    }
}

impl Game for Frontier {
    const NAME: &'static str = "frontier";
    type Query = Queries;
    type Action = Action;
    type Snapshot = Snapshot;
    type Bot = Unit;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        if setup.teams.len() != 2 {
            return Err(EngineError::Config(format!(
                "frontier needs exactly 2 teams, got {}",
                setup.teams.len()
            )));
        }
        let config = Config::from(setup.game_config.as_ref())?;
        let size = config.size;
        let mut map = Map::generate(size, setup.seed);
        let bases = [
            Coord::new(size / 4, size / 2),
            Coord::new(size - 1 - size / 4, size / 2),
        ];
        for base in bases {
            map.clear(base, 4);
            map.set(base, Tile::Base);
        }
        let mut game = Self {
            occupant: vec![0; size * size],
            config,
            bots: BotManager::new(),
            map,
            bases,
            commanders: [BotId(0); 2],
            ticks: 0,
            moved: false,
            spawned: false,
            changed: BTreeSet::new(),
            removed: Vec::new(),
            outcome: None,
        };
        // In step order, so the set's first team's commander steps first.
        for &team in &setup.teams {
            let base = bases[team.0 as usize];
            game.commanders[team.0 as usize] =
                game.place(team, Kind::Commander, Coord::new(base.x, base.y + 1));
        }
        Ok(game)
    }

    fn bots(&self) -> &BotManager<Unit> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<Unit> {
        &mut self.bots
    }

    /// Every unit in spawn order; a new tick starts a new delta.
    fn schedule(&mut self) -> Vec<BotId> {
        self.changed.clear();
        self.removed.clear();
        self.ticks += 1;
        self.bots.ids()
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::Observe(q) => {
                let me = self.bots[bot].pos;
                let r = self.config.vision;
                let last = self.map.size() - 1;
                let origin = Coord::new(me.x.saturating_sub(r), me.y.saturating_sub(r));
                let width = (me.x + r).min(last) - origin.x + 1;
                let height = (me.y + r).min(last) - origin.y + 1;
                let mut units = Vec::new();
                for y in origin.y..origin.y + height {
                    for x in origin.x..origin.x + width {
                        let o = self.occupant[self.idx(Coord::new(x, y))];
                        if o != 0 {
                            units.push(self.view(BotId(o - 1)));
                        }
                    }
                }
                Ok(q.reply(Observation {
                    tick: self.ticks - 1,
                    me: self.view(bot),
                    base: self.bases[self.bots[bot].team().0 as usize],
                    origin,
                    width,
                    height,
                    tiles: BASE64.encode(self.map.window(origin, width, height)),
                    units,
                }))
            }
        }
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        match action {
            Action::Move(a) => Ok(a.reply(self.move_unit(bot, a.dir)?)),
            Action::Spawn(a) => Ok(a.reply(self.spawn(bot)?)),
        }
    }

    fn end_step(&mut self, _bot: BotId) {
        self.moved = false;
        self.spawned = false;
    }

    fn bot_failed(&mut self, bot: BotId, failure: &BotFailure) {
        self.moved = false;
        self.spawned = false;
        let i = self.idx(self.bots[bot].pos);
        self.occupant[i] = 0;
        self.changed.remove(&bot);
        self.removed.push(bot.0);
        let team = self.bots[bot].team();
        if bot == self.commanders[team.0 as usize] && self.outcome.is_none() {
            self.outcome = Some(Outcome::forfeit(
                TeamId(1 - team.0),
                format!("the commander failed: {}", failure.detail()),
            ));
        }
    }

    fn end_tick(&mut self) {}

    fn status(&self) -> GameStatus {
        match &self.outcome {
            None => GameStatus::InProgress,
            Some(outcome) => GameStatus::Complete(outcome.clone()),
        }
    }

    fn snapshot(&self) -> Snapshot {
        let keyframe = self.ticks.is_multiple_of(self.config.keyframe);
        let units = if keyframe {
            self.bots
                .ids()
                .into_iter()
                .map(|id| self.view(id))
                .collect()
        } else {
            self.changed.iter().map(|&id| self.view(id)).collect()
        };
        Snapshot {
            tick: self.ticks,
            map: (self.ticks == 0).then(|| MapView {
                size: self.map.size(),
                tiles: BASE64.encode(self.map.bytes()),
                bases: self.bases.to_vec(),
            }),
            keyframe,
            units,
            removed: self.removed.clone(),
        }
    }
}
