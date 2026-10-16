//! What leaves the game: the `observe` reply and the replay snapshot.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::map::Coord;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Commander,
    Worker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UnitView {
    pub id: u64,
    pub team: u32,
    pub kind: Kind,
    pub pos: Coord,
}

/// What one unit sees: the square of tiles around it, clipped to the map, and the units
/// on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Observation {
    pub tick: u32,
    pub me: UnitView,
    /// Your team's base.
    pub base: Coord,
    /// Top-left tile of `tiles`.
    pub origin: Coord,
    pub width: usize,
    pub height: usize,
    /// `width * height` bytes, base64, row by row: 0 grass, 1 forest, 2 rock, 3 water,
    /// 4 ore, 5 base. Grass and forest are walkable.
    pub tiles: String,
    /// Units in view, you included.
    pub units: Vec<UnitView>,
}

/// The full map, in the initial state only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MapView {
    pub size: usize,
    /// `size * size` tile bytes as in [`Observation::tiles`], base64.
    pub tiles: String,
    /// Indexed by team.
    pub bases: Vec<Coord>,
}

/// The set after a tick, as a change from the one before: units that moved or appeared,
/// and units that left. A keyframe lists every unit instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    pub tick: u32,
    pub map: Option<MapView>,
    pub keyframe: bool,
    pub units: Vec<UnitView>,
    pub removed: Vec<u64>,
}
