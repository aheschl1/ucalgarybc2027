//! What leaves the game: query replies and the replay snapshot. Ids are plain
//! integers here, as the bot's handle has them.

use schemars::JsonSchema;
use serde::Serialize;

use crate::coord::Coord;
use crate::map::Environment;

/// What is on a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ItemView {
    Dino { team: u32, level: u32 },
    Fossil,
}

/// A bot as it sees itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnitView {
    Lab {
        /// Top-left of its four tiles.
        origin: Coord,
        health: u32,
    },
    Dino {
        pos: Coord,
        level: u32,
        health: u32,
        held: Option<ItemView>,
    },
}

/// Where a dropped item went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Dropped {
    /// Left on the tile.
    Placed,
    /// Into your lab; your team's deposited fossils now.
    Deposited { fossils: u32 },
}

/// The dino a spawn created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Spawned {
    pub bot_id: u64,
    pub at: Coord,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UnitEntry {
    pub id: u64,
    pub team: u32,
    #[serde(flatten)]
    pub unit: UnitView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TeamView {
    pub bones: u32,
    /// Deposited at the lab.
    pub fossils: u32,
}

/// The whole set after a tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Snapshot {
    pub tick: u32,
    /// Rows of tiles, `environment[y][x]`.
    pub environment: Vec<Vec<Environment>>,
    /// Live bots in step order.
    pub units: Vec<UnitEntry>,
    /// Fossils lying on the board.
    pub fossils: Vec<Coord>,
    /// Indexed by team.
    pub teams: Vec<TeamView>,
}
