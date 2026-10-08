pub const START_BONES: u32 = 50;
pub const LAB_HEALTH: u32 = 100;
pub const DINO_HEALTH: u32 = 10;
pub const DINO_MAX_HEALTH: u32 = 50;
pub const MERGES_PER_TURN: u32 = 1;
pub const ATTACKS_PER_TURN: u32 = 1;
pub const ATTACK_COST: u32 = 5;
pub const ATTACK_RANGE: usize = 1;
pub const EQUAL_LEVEL_DAMAGE: u32 = 2;
pub const MIN_DAMAGE: u32 = 1;
pub const SPAWN_COST: u32 = 10;
pub const MOVE_RANGE: usize = 1;
pub const ACTION_RANGE: usize = 1;
pub const BASE_INCOME: u32 = 1;
pub const INCOME_PER_FOSSIL: u32 = 1;

/// What one hit does: the level gap squared, half that (rounded down) when the
/// smaller dino hits, a fixed amount between equals, and never less than the minimum.
pub fn damage(attacker: u32, target: u32) -> u32 {
    let gap = attacker.abs_diff(target).pow(2);
    let damage = match attacker.cmp(&target) {
        std::cmp::Ordering::Equal => EQUAL_LEVEL_DAMAGE,
        std::cmp::Ordering::Greater => gap,
        std::cmp::Ordering::Less => gap / 2,
    };
    damage.max(MIN_DAMAGE)
}
