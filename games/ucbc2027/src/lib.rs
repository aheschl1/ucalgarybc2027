//! The UCBC 2027 competition game.

mod actions;
mod coord;
mod game;
mod grid;
mod map;
pub mod rules;
mod state;
mod unit;
mod view;

pub use coord::Coord;
pub use game::{Action, Queries, Ucbc2027};
pub use map::{Environment, proto};
pub use view::{Dropped, ItemView, Snapshot, Spawned, TeamView, UnitEntry, UnitView};
