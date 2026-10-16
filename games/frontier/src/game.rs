use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use schemars::JsonSchema;
use serde::Deserialize;
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, EngineError, Game, GameStatus, Outcome, QueryError,
    Request, SetSetup, TeamId,
};

use crate::config::Config;
use crate::content::{Building, Item, Recipe};
use crate::industry::Learned;
use crate::map::{Coord, Dir, Map};
use crate::rules::START;
use crate::view::{Hit, Kind, KnownView, Observation, Snapshot, Sown};
use crate::world::{Turn, Unit, World};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    /// What you see: tiles and what is on them within your vision, what you carry, the
    /// storage next to you, and what you heard.
    Observe(Request<(), Observation>),
    /// The recipes and buildings your team knows, in full. The rest are learned with
    /// `study`.
    Known(Request<(), KnownView>),
}

/// Each step a unit may move once, take one other action (all but `say`), and `say` once.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move one tile onto walkable ground with nothing on it, or through your team's gate;
    /// returns where you are.
    Move(Request<Heading, Coord>),
    /// Take one item from the tile in `dir`: wood from forest, stone from rock, ore and
    /// coal from veins. A tile runs out and turns to dirt.
    Gather(Request<Heading, Item>),
    /// Hand items to a teammate, into your team's storage, or onto the ground; returns
    /// how many moved.
    Put(Request<Transfer, u32>),
    /// Take items from a teammate, storage of either team (the other team's is theft), or
    /// the ground; returns how many moved.
    Take(Request<Transfer, u32>),
    /// Place a building your team knows, paying its cost from what you carry; work on your
    /// team's unfinished one; or repair a damaged one a little. Returns the work it still
    /// needs. Only the commander builds a base, and only once the old one is destroyed.
    Build(Request<Construct, u32>),
    /// Turn grass or dirt in `dir` into farmland.
    Till(Request<Heading, ()>),
    /// Plant a seed in empty farmland with water within 4 tiles (or a well's reach).
    Plant(Request<Seed, Sown>),
    /// Harvest a ripe crop, anyone's, if all it gives fits in what you carry; returns how
    /// many items. Fertile soil gives more, and each harvest wears it; it recovers while
    /// nothing grows on it.
    Harvest(Request<Heading, u32>),
    /// Start a recipe your team knows at the building in `dir` that makes it, from what
    /// it stores; the outputs appear in its storage when it ends. Returns that tick.
    Craft(Request<Order, u32>),
    /// Work the drill in `dir`: what it digs from the tile under it goes to you, up to
    /// its rate; a drill that burns fuel takes one from its storage. Returns how many.
    Operate(Request<Heading, u32>),
    /// Spend an item you carry at a research building in `dir`; your team learns every
    /// recipe and building that uses it.
    Study(Request<Lesson, Learned>),
    /// Hit what's on a tile next to you, or in range of your team's tower next to you:
    /// a unit or building of the other team, or else a crop, which is destroyed.
    Attack(Request<Coord, Hit>),
    /// Commander only, next to your base: a worker next to the base, paid from its
    /// storage. Returns its id.
    Spawn(Request<(), u64>),
    /// Send bytes (base64, at most 64) to teammates in range; they hear it next tick.
    Say(Request<Data, ()>),
    /// Leave a sign (base64, at most 64 bytes) on your tile for anyone who sees it;
    /// empty takes it down.
    Mark(Request<Data, ()>),
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Heading {
    pub dir: Dir,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Transfer {
    pub dir: Dir,
    pub item: Item,
    pub count: u32,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Seed {
    pub dir: Dir,
    pub seed: Item,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Order {
    pub dir: Dir,
    pub recipe: Recipe,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Lesson {
    pub dir: Dir,
    pub item: Item,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Construct {
    pub dir: Dir,
    pub building: Building,
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct Data {
    pub data: String,
}

fn decode(data: &str) -> Result<Vec<u8>, ActionError> {
    BASE64
        .decode(data)
        .map_err(|e| ActionError::Invalid(format!("not base64: {e}")))
}

pub struct Frontier {
    config: Config,
    world: World,
    commanders: [BotId; 2],
    /// Ticks begun; the snapshot after them is numbered this.
    ticks: u32,
    outcome: Option<Outcome>,
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
        let mut map = if config.flat {
            Map::flat(size)
        } else {
            Map::generate(size, setup.seed)
        };
        let bases = [
            Coord::new(size / 4, size / 2),
            Coord::new(size - 1 - size / 4, size / 2),
        ];
        map.settle(bases[0]);
        let mut world = World::new(map);
        for team in [TeamId(0), TeamId(1)] {
            let id = world.place_structure(team, Building::Base, bases[team.0 as usize]);
            world.structures.get_mut(&id).unwrap().work_left = 0;
            for (item, n) in START {
                world.structures.get_mut(&id).unwrap().stored.add(item, n);
            }
        }
        let mut commanders = [BotId(0); 2];
        // In step order, so the set's first team's commander steps first.
        for &team in &setup.teams {
            let base = bases[team.0 as usize];
            commanders[team.0 as usize] =
                world.place_unit(team, Kind::Commander, Coord::new(base.x, base.y + 1));
        }
        Ok(Self {
            config,
            world,
            commanders,
            ticks: 0,
            outcome: None,
        })
    }

    fn bots(&self) -> &ucbc_engine::BotManager<Unit> {
        &self.world.bots
    }

    fn bots_mut(&mut self) -> &mut ucbc_engine::BotManager<Unit> {
        &mut self.world.bots
    }

    /// Every unit in spawn order. A new tick starts a new delta, and what was said last
    /// tick is heard.
    fn schedule(&mut self) -> Vec<BotId> {
        let w = &mut self.world;
        w.changes = Default::default();
        w.heard = std::mem::take(&mut w.said);
        w.tick = self.ticks;
        w.finish_jobs();
        self.ticks += 1;
        w.bots.ids()
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::Observe(q) => Ok(q.reply(self.world.observe(bot, self.config.vision))),
            Queries::Known(q) => Ok(q.reply(self.world.known_view(self.world.bots[bot].team()))),
        }
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        let w = &mut self.world;
        match action {
            Action::Move(a) => Ok(a.reply(w.move_unit(bot, a.dir)?)),
            Action::Gather(a) => Ok(a.reply(w.gather(bot, a.dir)?)),
            Action::Put(a) => Ok(a.reply(w.put(bot, a.dir, a.item, a.count)?)),
            Action::Take(a) => Ok(a.reply(w.take(bot, a.dir, a.item, a.count)?)),
            Action::Build(a) => Ok(a.reply(w.build(bot, a.dir, a.building)?)),
            Action::Till(a) => Ok(a.reply(w.till(bot, a.dir)?)),
            Action::Plant(a) => Ok(a.reply(w.plant(bot, a.dir, a.seed)?)),
            Action::Harvest(a) => Ok(a.reply(w.harvest(bot, a.dir)?)),
            Action::Craft(a) => Ok(a.reply(w.craft(bot, a.dir, a.recipe)?)),
            Action::Operate(a) => Ok(a.reply(w.operate(bot, a.dir)?)),
            Action::Study(a) => Ok(a.reply(w.study(bot, a.dir, a.item)?)),
            Action::Attack(a) => {
                let (hit, killed) = w.attack(bot, *a)?;
                if killed.is_some_and(|id| self.commanders.contains(&id)) {
                    let winner = w.bots[bot].team();
                    self.outcome =
                        Some(Outcome::win(winner).with_detail("killed the other commander"));
                }
                Ok(a.reply(hit))
            }
            Action::Spawn(a) => Ok(a.reply(w.spawn(bot, self.config.units)?)),
            Action::Say(a) => Ok(a.reply(w.say(bot, decode(&a.data)?)?)),
            Action::Mark(a) => Ok(a.reply(w.mark(bot, decode(&a.data)?)?)),
        }
    }

    fn end_step(&mut self, _bot: BotId) {
        self.world.turn = Turn::default();
    }

    fn bot_failed(&mut self, bot: BotId, failure: &BotFailure) {
        self.world.turn = Turn::default();
        let team = self.world.bots[bot].team();
        self.world.remove_unit(bot);
        if bot == self.commanders[team.0 as usize] && self.outcome.is_none() {
            self.outcome = Some(Outcome::forfeit(
                TeamId(1 - team.0),
                format!("the commander failed: {}", failure.detail()),
            ));
        }
    }

    /// Meals. A team whose commander starved loses; both at once, a draw.
    fn end_tick(&mut self) {
        let dead = self.world.feed();
        let starved: Vec<u32> = dead
            .iter()
            .filter(|&&id| self.commanders.contains(&id))
            .map(|&id| self.world.bots[id].team().0)
            .collect();
        for id in dead {
            self.world.remove_unit(id);
            self.world.bots.remove(id);
        }
        self.outcome = match starved[..] {
            [] => return,
            [team] => {
                Some(Outcome::win(TeamId(1 - team)).with_detail("the other commander starved"))
            }
            _ => Some(Outcome::draw("both commanders starved")),
        };
    }

    fn status(&self) -> GameStatus {
        match &self.outcome {
            None => GameStatus::InProgress,
            Some(outcome) => GameStatus::Complete(outcome.clone()),
        }
    }

    fn snapshot(&self) -> Snapshot {
        self.world
            .snapshot(self.ticks, self.ticks.is_multiple_of(self.config.keyframe))
    }
}
