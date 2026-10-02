//! Game balance numbers. Placeholders until the design doc settles them.

use crate::unit::Dino;

pub const START_BONES: u32 = 50;
pub const LAB_HEALTH: u32 = 100;
pub const DINO_HEALTH: u32 = 10;
pub const SPAWN_COST: u32 = 10;
pub const MOVE_RANGE: usize = 1;

/// What a dino can do this turn.
pub struct Stats {
    /// How far one move may go.
    pub move_range: usize,
}

/// The same for every dino until levels and artifacts change it.
pub fn stats(_dino: &Dino) -> Stats {
    Stats {
        move_range: MOVE_RANGE,
    }
}
/// Bones each team earns per tick.
pub const BASE_INCOME: u32 = 1;
