//! Game balance numbers. Placeholders until the design doc settles them.

use crate::state::TeamState;
use crate::unit::Dino;

pub const START_BONES: u32 = 50;
pub const LAB_HEALTH: u32 = 100;
pub const DINO_HEALTH: u32 = 10;
pub const SPAWN_COST: u32 = 10;
pub const MOVE_RANGE: usize = 1;
pub const ACTION_RANGE: usize = 1;
pub const BASE_INCOME: u32 = 1;
pub const INCOME_PER_FOSSIL: u32 = 1;

/// What a dino can do this turn.
pub struct Stats {
    /// How far one move may go.
    pub move_range: usize,
    /// How far away a grab or drop may reach.
    pub action_range: usize,
}

/// The same for every dino until levels and artifacts change it.
pub fn stats(_dino: &Dino) -> Stats {
    Stats {
        move_range: MOVE_RANGE,
        action_range: ACTION_RANGE,
    }
}

/// Bones a team earns per tick.
pub fn income(team: &TeamState) -> u32 {
    BASE_INCOME + INCOME_PER_FOSSIL * team.fossils
}
