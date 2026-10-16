//! Frontier: farming, industry, and war on a large generated map. See docs/frontier.md.

mod config;
pub mod content;
mod farm;
mod game;
mod map;
pub mod rules;
mod view;
mod world;

pub use config::Config;
pub use content::{Building, Crop, Item};
pub use game::{Action, Construct, Data, Frontier, Heading, Queries, Seed, Transfer};
pub use map::{Coord, Dir, Map, Tile};
pub use view::{Kind, MapView, Me, Message, Observation, Sign, Snapshot, Sown, Stack, Store};
