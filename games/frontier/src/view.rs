//! What leaves the game: the `observe` reply and the replay snapshot. Units, buildings,
//! and ground items go as flat integer rows, which a bot decodes only if it needs to.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::BotId;

use crate::content::Item;
use crate::map::{Coord, Dir};
use crate::world::{Items, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Commander,
    Worker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stack {
    pub item: Item,
    pub count: u32,
}

fn stacks(items: &Items) -> Vec<Stack> {
    items
        .0
        .iter()
        .map(|(&item, &count)| Stack { item, count })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Me {
    pub id: u64,
    pub team: u32,
    pub kind: Kind,
    pub pos: Coord,
    pub carrying: Vec<Stack>,
}

/// A finished storage building of your team that `put` and `take` reach, with what it
/// holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Store {
    pub id: u64,
    pub pos: Coord,
    pub stored: Vec<Stack>,
}

/// Bytes on a tile or over the radio, base64.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sign {
    pub pos: Coord,
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Message {
    pub sender: u64,
    pub data: String,
}

/// What one unit sees: the square of tiles around it, clipped to the map, and what is
/// on them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    pub tick: u32,
    pub me: Me,
    /// Your team's base.
    pub base: Coord,
    /// Top-left tile of `tiles`.
    pub origin: Coord,
    pub width: usize,
    pub height: usize,
    /// `width * height` bytes, base64, row by row: 0 grass, 1 forest, 2 rock, 3 water,
    /// 4 dirt, 5 coal, 6 copper, 7 iron. Grass, forest, and dirt are walkable.
    pub tiles: String,
    /// `[id, team, kind, x, y]` per unit in view, you included; kind 0 commander, 1 worker.
    pub units: Vec<u64>,
    /// `[id, team, building, x, y, hp, work_left]` per building in view; `building` is an
    /// index into the building ids.
    pub buildings: Vec<u64>,
    /// `[x, y, item, count]` per kind of item lying on a tile in view.
    pub piles: Vec<u64>,
    pub signs: Vec<Sign>,
    /// Your team's storage in the four directions next to you.
    pub stores: Vec<Store>,
    /// What teammates said last tick that reached you.
    pub heard: Vec<Message>,
}

/// The full map, in the initial state only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MapView {
    pub size: usize,
    /// `size * size` tile bytes as in [`Observation::tiles`], base64.
    pub tiles: String,
}

/// The set after a tick, as a change from the one before; a keyframe lists every unit,
/// building, pile, and sign instead. Tiles are changes only: the map is the initial
/// state's with every later change applied. Rows are as in [`Observation`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    pub tick: u32,
    pub map: Option<MapView>,
    pub keyframe: bool,
    /// `[id, team, kind, x, y, carried]`
    pub units: Vec<u64>,
    pub removed: Vec<u64>,
    /// `[x, y, tile]` for tiles that changed.
    pub tiles: Vec<u64>,
    pub buildings: Vec<u64>,
    /// `[x, y, item, count]`. A tile's rows replace whatever was on it; a single row with
    /// a count of 0 means nothing is.
    pub piles: Vec<u64>,
    /// An empty sign was taken down.
    pub signs: Vec<Sign>,
}

impl World {
    fn unit_row(&self, id: BotId, out: &mut Vec<u64>) {
        let u = &self.bots[id];
        let team = u64::from(u.team().0);
        out.extend([id.0, team, u.kind as u64, u.pos.x as u64, u.pos.y as u64]);
    }

    fn structure_row(&self, id: u64, out: &mut Vec<u64>) {
        let s = &self.structures[&id];
        let (x, y) = (s.pos.x as u64, s.pos.y as u64);
        let team = u64::from(s.team.0);
        out.extend([
            id,
            team,
            s.kind as u64,
            x,
            y,
            u64::from(s.hp),
            u64::from(s.work_left),
        ]);
    }

    fn pile_rows(&self, at: Coord, out: &mut Vec<u64>) {
        let (x, y) = (at.x as u64, at.y as u64);
        if let Some(pile) = self.piles.get(&(at.y, at.x)) {
            for (&item, &n) in &pile.0 {
                out.extend([x, y, item as u64, u64::from(n)]);
            }
        }
    }

    fn sign(&self, at: Coord) -> Sign {
        let data = self.signs.get(&(at.y, at.x)).map_or(&[][..], Vec::as_slice);
        Sign {
            pos: at,
            data: BASE64.encode(data),
        }
    }

    pub fn observe(&self, bot: BotId, tick: u32, vision: usize) -> Observation {
        let unit = &self.bots[bot];
        let me = unit.pos;
        let last = self.map.size() - 1;
        let origin = Coord::new(me.x.saturating_sub(vision), me.y.saturating_sub(vision));
        let width = (me.x + vision).min(last) - origin.x + 1;
        let height = (me.y + vision).min(last) - origin.y + 1;
        let (x0, x1) = (origin.x, origin.x + width - 1);
        let (mut units, mut buildings, mut piles, mut signs) = (vec![], vec![], vec![], vec![]);
        for y in origin.y..origin.y + height {
            for x in x0..=x1 {
                let at = Coord::new(x, y);
                if let Some(id) = self.unit_on(at) {
                    self.unit_row(id, &mut units);
                }
                if let Some(id) = self.structure_on(at) {
                    self.structure_row(id, &mut buildings);
                }
            }
            for &(y, x) in self.piles.range((y, x0)..=(y, x1)).map(|(k, _)| k) {
                self.pile_rows(Coord::new(x, y), &mut piles);
            }
            signs.extend(
                self.signs
                    .range((y, x0)..=(y, x1))
                    .map(|(&(y, x), _)| self.sign(Coord::new(x, y))),
            );
        }
        let team = unit.team();
        let stores = Dir::ALL
            .into_iter()
            .filter_map(|dir| self.structure_on(self.map.step(me, dir)?))
            .map(|id| (id, &self.structures[&id]))
            .filter(|(_, s)| s.team == team && s.capacity().is_some())
            .map(|(id, s)| Store {
                id,
                pos: s.pos,
                stored: stacks(&s.stored),
            })
            .collect();
        let heard = self
            .heard
            .iter()
            .filter(|said| self.hears(bot, said))
            .map(|said| Message {
                sender: said.from.0,
                data: BASE64.encode(&said.data),
            })
            .collect();
        Observation {
            tick,
            me: Me {
                id: bot.0,
                team: team.0,
                kind: unit.kind,
                pos: me,
                carrying: stacks(&unit.carrying),
            },
            base: self.base(team).pos,
            origin,
            width,
            height,
            tiles: BASE64.encode(self.map.window(origin, width, height)),
            units,
            buildings,
            piles,
            signs,
            stores,
            heard,
        }
    }

    pub fn snapshot(&self, tick: u32, keyframe: bool) -> Snapshot {
        let c = &self.changes;
        let unit_ids: Vec<BotId> = if keyframe {
            self.bots.ids()
        } else {
            c.units.iter().copied().collect()
        };
        let mut units = Vec::new();
        for id in unit_ids {
            self.unit_row(id, &mut units);
            units.push(u64::from(self.bots[id].carrying.total()));
        }
        let structure_ids: Vec<u64> = if keyframe {
            self.structures.keys().copied().collect()
        } else {
            c.structures.iter().copied().collect()
        };
        let mut buildings = Vec::new();
        for id in structure_ids {
            self.structure_row(id, &mut buildings);
        }
        let pile_at: Vec<Coord> = if keyframe {
            self.piles.keys().map(|&(y, x)| Coord::new(x, y)).collect()
        } else {
            c.piles.iter().copied().collect()
        };
        let mut piles = Vec::new();
        for at in pile_at {
            let before = piles.len();
            self.pile_rows(at, &mut piles);
            if piles.len() == before {
                piles.extend([at.x as u64, at.y as u64, 0, 0]);
            }
        }
        let sign_at: Vec<Coord> = if keyframe {
            self.signs.keys().map(|&(y, x)| Coord::new(x, y)).collect()
        } else {
            c.signs.iter().copied().collect()
        };
        Snapshot {
            tick,
            map: (tick == 0).then(|| MapView {
                size: self.map.size(),
                tiles: BASE64.encode(self.map.bytes()),
            }),
            keyframe,
            units,
            removed: c.removed.clone(),
            tiles: c
                .tiles
                .iter()
                .flat_map(|&at| [at.x as u64, at.y as u64, self.map.tile(at) as u64])
                .collect(),
            buildings,
            piles,
            signs: sign_at.into_iter().map(|at| self.sign(at)).collect(),
        }
    }
}
