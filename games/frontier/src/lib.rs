//! Frontier, Phase 0: a scaling prototype. A generated map, a base and commander per
//! team, units that move and observe, and delta snapshots. See docs/frontier.md.

mod config;
mod game;
mod map;
mod view;

pub use config::Config;
pub use game::{Action, Frontier, Heading, Queries};
pub use map::{Coord, Dir, Map, Tile};
pub use view::{Kind, MapView, Observation, Snapshot, UnitView};
