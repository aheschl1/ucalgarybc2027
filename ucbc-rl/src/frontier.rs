//! Frontier's `observe` reply as tensors, and action ids as actions. Everything comes from
//! the reply, so the policy knows no more than a sandboxed bot. Team 1 sees and acts
//! mirrored left to right, so one policy plays both sides of the mirrored map.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::{Value, json};
use ucbc_frontier::rules::{CARRY, FIRST_MEAL, HP};
use ucbc_frontier::{Building, Dir, Item, Kind, Observation, Recipe};

/// One byte a cell, each plane `side * side`, row by row from the unit's top left.
pub mod plane {
    /// Tile id + 1; 0 off the map.
    pub const TILE: usize = 0;
    /// 1 own commander, 2 own worker, 3 enemy commander, 4 enemy worker.
    pub const UNIT: usize = 1;
    pub const UNIT_HP: usize = 2;
    /// Building id + 1.
    pub const BUILDING: usize = 3;
    /// 1 own, 2 own site, 3 enemy, 4 enemy site.
    pub const BUILDING_STATE: usize = 4;
    /// Out of 255 of the building's full health.
    pub const BUILDING_HP: usize = 5;
    /// Items lying there, up to 255.
    pub const PILE: usize = 6;
    /// 1 + 2 * crop id + ripe.
    pub const CROP: usize = 7;
    /// Fertility + 1 on farmland.
    pub const SOIL: usize = 8;
    pub const COUNT: usize = 9;
}

pub const VERBS: [&str; 13] = [
    "none", "gather", "put", "take", "build", "till", "plant", "harvest", "craft", "operate",
    "study", "attack", "spawn",
];

/// Attack targets are offsets in a square this wide around the unit.
pub const REACH: usize = 15;
pub const COUNTS: [u32; 3] = [1, 5, CARRY];

/// A step is one id per head, in this order. A head a verb doesn't use is ignored. Left
/// and right are in the unit's frame: mirrored for team 1.
pub mod head {
    /// 0 stays put; 1-4 step up, down, left, right.
    pub const MOVE: usize = 0;
    /// The step's one other action: an index into [`VERBS`](super::VERBS); 0 is none.
    pub const VERB: usize = 1;
    /// The side the verb acts on: up, down, left, right. Every verb but attack and spawn.
    pub const DIR: usize = 2;
    /// What put, take, and study move, and what plant sows: an index into `Item::ALL`.
    pub const ITEM: usize = 3;
    /// How many put or take move: an index into [`COUNTS`](super::COUNTS).
    pub const COUNT: usize = 4;
    /// What build places or works on: an index into `Building::ALL`.
    pub const BUILDING: usize = 5;
    /// What craft starts: an index into `Recipe::ALL`.
    pub const RECIPE: usize = 6;
    /// The cell attack hits: row by row over the [`REACH`](super::REACH) square centred
    /// on the unit, in the frame it was encoded in.
    pub const TARGET: usize = 7;
    /// 0 acts then moves, 1 moves then acts. A verb's side is from where the unit stands
    /// when it acts.
    pub const ORDER: usize = 8;
}

/// Choices per head, indexed by the [`head`] constants.
pub const HEADS: [usize; 9] = [
    5,
    VERBS.len(),
    4,
    Item::ALL.len(),
    COUNTS.len(),
    Building::ALL.len(),
    Recipe::ALL.len(),
    REACH * REACH,
    2,
];

const CARRIED: usize = 9;
const STORE: usize = 3 + 2 * Item::ALL.len();
/// Commander, hp, ticks to hunger, x, y, base seen and its offset, clock, what it
/// carries, then the storage on each side.
pub const VECTOR: usize = CARRIED + Item::ALL.len() + 4 * STORE;

pub struct Codec {
    pub vision: usize,
    pub max_ticks: u32,
}

impl Codec {
    pub fn side(&self) -> usize {
        2 * self.vision + 1
    }

    pub fn grid_len(&self) -> usize {
        plane::COUNT * self.side() * self.side()
    }

    /// Fills `grid` ([`Codec::grid_len`]) and `vector` ([`VECTOR`]).
    pub fn encode(
        &self,
        obs: &Value,
        grid: &mut [u8],
        vector: &mut [f32],
    ) -> Result<Context, String> {
        let obs = Observation::deserialize(obs).map_err(|e| e.to_string())?;
        let tiles = BASE64.decode(&obs.tiles).map_err(|e| e.to_string())?;
        let at = Context {
            mirror: obs.me.team == 1,
            x: obs.me.pos.x,
            y: obs.me.pos.y,
            size: obs.size,
        };
        let frame = Frame {
            at,
            vision: self.vision,
            side: self.side(),
        };
        grid.fill(0);
        vector.fill(0.0);
        frame.paint(&obs, &tiles, grid);
        frame.vector(&obs, self.max_ticks, vector);
        Ok(at)
    }
}

/// Where a unit stood when it was encoded, to place its actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub mirror: bool,
    pub x: usize,
    pub y: usize,
    pub size: usize,
}

impl Context {
    /// The step's actions from one id per [`head`]: the verb and the move in its order,
    /// each left out when it's none.
    pub fn decode(&self, ids: &[usize; 9]) -> Vec<Value> {
        let [mv, verb, dir, item, count, building, recipe, target, order] = *ids;
        let dir_of = |i: usize| match (self.mirror, Dir::ALL[i]) {
            (true, Dir::Left) => Dir::Right,
            (true, Dir::Right) => Dir::Left,
            (_, dir) => dir,
        };
        let (dir, item) = (dir_of(dir), Item::ALL[item]);
        let action = match VERBS[verb] {
            "gather" | "till" | "harvest" | "operate" => Some(json!({"dir": dir})),
            "put" | "take" => Some(json!({"dir": dir, "item": item, "count": COUNTS[count]})),
            "build" => Some(json!({"dir": dir, "building": Building::ALL[building]})),
            "plant" => Some(json!({"dir": dir, "seed": item})),
            "craft" => Some(json!({"dir": dir, "recipe": Recipe::ALL[recipe]})),
            "study" => Some(json!({"dir": dir, "item": item})),
            "attack" => self.target(target).map(|(x, y)| json!({"x": x, "y": y})),
            "spawn" => Some(json!({})),
            _ => None,
        };
        let action = action.map(|mut action| {
            action["type"] = json!(VERBS[verb]);
            action
        });
        let step = (mv > 0).then(|| json!({"type": "move", "dir": dir_of(mv - 1)}));
        let [first, second] = if order == 1 {
            [step, action]
        } else {
            [action, step]
        };
        first.into_iter().chain(second).collect()
    }

    fn target(&self, i: usize) -> Option<(usize, usize)> {
        let r = (REACH / 2) as isize;
        let dx = (i % REACH) as isize - r;
        let dy = (i / REACH) as isize - r;
        let dx = if self.mirror { -dx } else { dx };
        let x = self.x.checked_add_signed(dx).filter(|&x| x < self.size)?;
        let y = self.y.checked_add_signed(dy).filter(|&y| y < self.size)?;
        Some((x, y))
    }
}

/// The unit's frame: centred on it, mirrored for team 1.
struct Frame {
    at: Context,
    vision: usize,
    side: usize,
}

impl Frame {
    /// The index of world `(x, y)` in a plane, if it's in the square.
    fn cell(&self, x: u64, y: u64) -> Option<usize> {
        let v = self.vision as i64;
        let col = x as i64 - self.at.x as i64;
        let col = if self.at.mirror { -col } else { col } + v;
        let row = y as i64 - self.at.y as i64 + v;
        let side = self.side as i64;
        ((0..side).contains(&col) && (0..side).contains(&row)).then(|| (row * side + col) as usize)
    }

    fn paint(&self, obs: &Observation, tiles: &[u8], grid: &mut [u8]) {
        let area = self.side * self.side;
        let mut set = |plane: usize, x: u64, y: u64, value: u8| {
            if let Some(c) = self.cell(x, y) {
                grid[plane * area + c] = value;
            }
        };
        let team = u64::from(obs.me.team);
        for (i, &tile) in tiles.iter().enumerate() {
            let x = (obs.origin.x + i % obs.width) as u64;
            let y = (obs.origin.y + i / obs.width) as u64;
            set(plane::TILE, x, y, tile + 1);
        }
        for &[_, t, kind, x, y, hp] in obs.units.as_chunks::<6>().0 {
            set(plane::UNIT, x, y, 1 + kind as u8 + 2 * u8::from(t != team));
            set(plane::UNIT_HP, x, y, hp.min(255) as u8);
        }
        for &[_, t, kind, x, y, hp, work_left] in obs.buildings.as_chunks::<7>().0 {
            let full = u64::from(Building::ALL[kind as usize].def().hp.max(1));
            let state = 1 + u8::from(work_left > 0) + 2 * u8::from(t != team);
            set(plane::BUILDING, x, y, kind as u8 + 1);
            set(plane::BUILDING_STATE, x, y, state);
            set(plane::BUILDING_HP, x, y, (hp * 255 / full).min(255) as u8);
        }
        for &[x, y, crop, ripe_at] in obs.crops.as_chunks::<4>().0 {
            let ripe = u8::from(ripe_at <= u64::from(obs.tick));
            set(plane::CROP, x, y, 1 + 2 * crop as u8 + ripe);
        }
        for &[x, y, fertility] in obs.soil.as_chunks::<3>().0 {
            set(plane::SOIL, x, y, fertility as u8 + 1);
        }
        // A tile's piles are a row per item.
        for &[x, y, _, n] in obs.piles.as_chunks::<4>().0 {
            if let Some(c) = self.cell(x, y) {
                let cell = &mut grid[plane::PILE * area + c];
                *cell = (u64::from(*cell) + n).min(255) as u8;
            }
        }
    }

    fn vector(&self, obs: &Observation, max_ticks: u32, out: &mut [f32]) {
        let size = obs.size as f32;
        let me = obs.me.pos;
        let flip = |dx: i64| if self.at.mirror { -dx } else { dx };
        let x = if self.at.mirror {
            obs.size - 1 - me.x
        } else {
            me.x
        };
        let hungry = obs.me.hungry_at.saturating_sub(obs.tick) as f32 / FIRST_MEAL as f32;
        out[0] = f32::from(u8::from(obs.me.kind == Kind::Commander));
        out[1] = obs.me.hp as f32 / HP as f32;
        out[2] = hungry.min(1.0);
        out[3] = x as f32 / size;
        out[4] = me.y as f32 / size;
        if let Some(base) = obs.base {
            out[5] = 1.0;
            out[6] = flip(base.x as i64 - me.x as i64) as f32 / size;
            out[7] = (base.y as f32 - me.y as f32) / size;
        }
        out[8] = obs.tick as f32 / max_ticks.max(1) as f32;
        for s in &obs.me.carrying {
            out[CARRIED + s.item as usize] = s.count as f32 / CARRY as f32;
        }
        let stores = CARRIED + Item::ALL.len();
        for store in &obs.stores {
            let dx = flip(store.pos.x as i64 - me.x as i64);
            let dy = store.pos.y as i64 - me.y as i64;
            // In Dir::ALL's order: up, down, left, right.
            let Some(side) = [(0, -1), (0, 1), (-1, 0), (1, 0)]
                .iter()
                .position(|&d| d == (dx, dy))
            else {
                continue;
            };
            let o = &mut out[stores + side * STORE..][..STORE];
            o[0] = 1.0;
            o[1] = f32::from(u8::from(!store.owed.is_empty()));
            o[2] = f32::from(u8::from(store.job.is_some()));
            for s in &store.stored {
                o[3 + s.item as usize] = squash(s.count);
            }
            for s in &store.owed {
                o[3 + Item::ALL.len() + s.item as usize] = squash(s.count);
            }
        }
    }
}

/// A count in [0, 1): a half at 10.
fn squash(n: u32) -> f32 {
    n as f32 / (n as f32 + 10.0)
}
