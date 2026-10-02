//! The UCBC 2027 competition game.

mod bot;
mod game;
mod state;

pub use bot::BotType;
pub use game::{Action, Coord, Queries, Ucbc2027};
pub use state::{Environment, Item, State, Tile};
