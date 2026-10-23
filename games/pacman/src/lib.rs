//! Pac-Man on the ucbc engine. The set's first team plays Pac-Man, the other the
//! ghosts; every bot has one query, `view`, and one action, `move`.

mod game;
mod map;
pub mod rules;
mod view;

pub use game::{Action, Heading, Pacman, Queries};
pub use map::{Coord, Dir, Map, MapError, Tile};
pub use view::{AgentView, Role, Snapshot, View};
