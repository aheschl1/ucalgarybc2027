//! What leaves the game: the `observe` reply and the replay snapshot. Units, buildings,
//! and ground items go as flat integer rows, which a bot decodes only if it needs to.

use std::collections::BTreeMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{BotId, TeamId};

use crate::content::{Building, Crop, DrillDef, Item, Recipe};
use crate::map::{Coord, Dir};
use crate::world::World;

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

fn stacks(items: &BTreeMap<Item, u32>) -> Vec<Stack> {
    items
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
    pub hp: u32,
    /// The tick you next eat, from what you carry; with nothing to eat, you lose health
    /// every tick from then on.
    pub hungry_at: u32,
}

/// Next to you in the four directions: a finished storage building of your team, with
/// what it holds and its job, or a site of your team, with what it's still owed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Store {
    pub id: u64,
    pub pos: Coord,
    pub stored: Vec<Stack>,
    /// The recipe running, and the tick it ends.
    pub job: Option<JobView>,
    /// What its site needs delivered, with `put`, before work on it can start.
    pub owed: Vec<Stack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobView {
    pub recipe: Recipe,
    pub done_at: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecipeView {
    pub recipe: Recipe,
    /// The building it's made at.
    pub at: Building,
    pub inputs: Vec<Stack>,
    pub outputs: Vec<Stack>,
    pub ticks: u32,
}

/// A building's cost and what it does; behaviours it lacks are absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuildingView {
    pub building: Building,
    pub upgrades: Option<Building>,
    pub hp: u32,
    pub cost: Vec<Stack>,
    pub work: u32,
    pub storage: Option<u32>,
    pub radio: Option<usize>,
    pub water: Option<usize>,
    pub research: bool,
    pub drill: Option<DrillDef>,
    /// Its team walks through it.
    pub gate: bool,
    /// Units next to it attack within this range of it, and take half damage.
    pub tower: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnownView {
    pub recipes: Vec<RecipeView>,
    pub buildings: Vec<BuildingView>,
}

/// What an `attack` did, and the health its target has left; 0 means it died or fell.
/// A crop attacked is destroyed outright.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hit {
    pub damage: u32,
    pub left: u32,
}

/// What `plant` sowed, and the tick it ripens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sown {
    pub crop: Crop,
    pub ripe_at: u32,
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
    /// The map is `size` x `size`, mirrored left to right.
    pub size: usize,
    pub me: Me,
    /// Your team's base, unless it was destroyed and isn't rebuilt yet.
    pub base: Option<Coord>,
    /// Top-left tile of `tiles`.
    pub origin: Coord,
    pub width: usize,
    pub height: usize,
    /// `width * height` bytes, base64, row by row: 0 grass, 1 forest, 2 rock, 3 water,
    /// 4 dirt, 5 coal, 6 copper, 7 iron, 8 farmland. Grass, forest, dirt, and farmland
    /// are walkable.
    pub tiles: String,
    /// `[id, team, kind, x, y, hp]` per unit in view, you included; kind 0 commander,
    /// 1 worker.
    pub units: Vec<u64>,
    /// `[id, team, building, x, y, hp, work_left]` per building in view; `building` is an
    /// index into the building ids.
    pub buildings: Vec<u64>,
    /// `[x, y, item, count]` per kind of item lying on a tile in view.
    pub piles: Vec<u64>,
    /// `[x, y, crop, ripe_at]` per crop in view; `crop` is an index into the crop ids.
    pub crops: Vec<u64>,
    /// `[x, y, fertility]` per farmland tile in view, out of 100.
    pub soil: Vec<u64>,
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
    /// `[id, team, kind, x, y, hp, carried]`
    pub units: Vec<u64>,
    pub removed: Vec<u64>,
    /// `[x, y, tile]` for tiles that changed.
    pub tiles: Vec<u64>,
    pub buildings: Vec<u64>,
    /// Buildings destroyed.
    pub demolished: Vec<u64>,
    /// `[x, y, item, count]`. A tile's rows replace whatever was on it; a single row with
    /// a count of 0 means nothing is.
    pub piles: Vec<u64>,
    /// `[x, y, crop, ripe_at]` per crop that changed.
    pub crops: Vec<u64>,
    /// `[x, y]` per tile whose crop is gone.
    pub cleared: Vec<u64>,
    /// An empty sign was taken down.
    pub signs: Vec<Sign>,
}

impl World {
    fn unit_row(&self, id: BotId, out: &mut Vec<u64>) {
        let u = &self.bots[id];
        let team = u64::from(u.team().0);
        let (x, y) = (u.pos.x as u64, u.pos.y as u64);
        out.extend([id.0, team, u.kind as u64, x, y, u64::from(u.hp)]);
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

    fn crop_row(&self, at: Coord, out: &mut Vec<u64>) {
        if let Some(p) = self.crops.get(&(at.y, at.x)) {
            out.extend([
                at.x as u64,
                at.y as u64,
                p.crop as u64,
                u64::from(p.ripe_at),
            ]);
        }
    }

    fn sign(&self, at: Coord) -> Sign {
        let data = self.signs.get(&(at.y, at.x)).map_or(&[][..], Vec::as_slice);
        Sign {
            pos: at,
            data: BASE64.encode(data),
        }
    }

    pub fn observe(&self, bot: BotId, vision: usize) -> Observation {
        let unit = &self.bots[bot];
        let me = unit.pos;
        let last = self.map.size() - 1;
        let origin = Coord::new(me.x.saturating_sub(vision), me.y.saturating_sub(vision));
        let width = (me.x + vision).min(last) - origin.x + 1;
        let height = (me.y + vision).min(last) - origin.y + 1;
        let (x0, x1) = (origin.x, origin.x + width - 1);
        let (mut units, mut buildings, mut piles, mut signs) = (vec![], vec![], vec![], vec![]);
        let (mut crops, mut soil) = (vec![], vec![]);
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
            for &(y, x) in self.soil.range((y, x0)..=(y, x1)).map(|(k, _)| k) {
                let at = Coord::new(x, y);
                self.crop_row(at, &mut crops);
                soil.extend([x as u64, y as u64, u64::from(self.fertility(at))]);
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
            .filter(|(_, s)| s.team == team && (s.capacity().is_some() || s.owed.total() > 0))
            .map(|(id, s)| Store {
                id,
                pos: s.pos,
                stored: stacks(&s.stored.0),
                job: s.job.as_ref().map(|j| JobView {
                    recipe: j.recipe,
                    done_at: j.done_at,
                }),
                owed: stacks(&s.owed.0),
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
            tick: self.tick,
            size: self.map.size(),
            me: Me {
                id: bot.0,
                team: team.0,
                kind: unit.kind,
                pos: me,
                carrying: stacks(&unit.carrying.0),
                hp: unit.hp,
                hungry_at: unit.hungry_at,
            },
            base: self.base(team).filter(|b| b.work_left == 0).map(|b| b.pos),
            origin,
            width,
            height,
            tiles: BASE64.encode(self.map.window(origin, width, height)),
            units,
            buildings,
            piles,
            crops,
            soil,
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
        let crop_at: Vec<Coord> = if keyframe {
            self.crops.keys().map(|&(y, x)| Coord::new(x, y)).collect()
        } else {
            c.crops.iter().copied().collect()
        };
        let (mut crops, mut cleared) = (vec![], vec![]);
        for at in crop_at {
            let before = crops.len();
            self.crop_row(at, &mut crops);
            if crops.len() == before {
                cleared.extend([at.x as u64, at.y as u64]);
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
            demolished: c.demolished.clone(),
            piles,
            crops,
            cleared,
            signs: sign_at.into_iter().map(|at| self.sign(at)).collect(),
        }
    }

    pub fn known_view(&self, team: TeamId) -> KnownView {
        let known = self.known(team);
        let recipes = known.recipes.iter().map(|&recipe| {
            let r = recipe.def();
            RecipeView {
                recipe,
                at: r.at,
                inputs: stacks(&r.inputs),
                outputs: stacks(&r.outputs),
                ticks: r.ticks,
            }
        });
        let buildings = known.buildings.iter().map(|&building| {
            let b = building.def();
            BuildingView {
                building,
                upgrades: b.upgrades,
                hp: b.hp,
                cost: stacks(&b.cost),
                work: b.work,
                storage: b.storage,
                radio: b.radio,
                water: b.water,
                research: b.research,
                drill: b.drill.clone(),
                gate: b.gate,
                tower: b.tower,
            }
        });
        KnownView {
            recipes: recipes.collect(),
            buildings: buildings.collect(),
        }
    }
}
