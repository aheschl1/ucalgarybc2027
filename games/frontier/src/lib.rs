//! Frontier: farming, industry, and war on a large generated map. See docs/frontier.md.

mod config;
pub mod content;
mod farm;
mod game;
mod industry;
mod map;
pub mod rules;
mod view;
mod war;
mod world;

pub use config::Config;
pub use content::{Building, Crop, Item, Recipe};
pub use game::{
    Action, Construct, Data, Frontier, Heading, Lesson, Order, Queries, Seed, Transfer,
};
pub use industry::Learned;
pub use map::{Coord, Dir, Map, Tile};
pub use view::{
    BuildingView, JobView, Kind, KnownView, MapView, Me, Message, Observation, RecipeView, Sign,
    Snapshot, Sown, Stack, Store,
};
