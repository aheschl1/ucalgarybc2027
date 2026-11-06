//! The numbers to tune.

use crate::content::Item;

/// Items a unit carries, of any kind together.
pub const CARRY: u32 = 20;
/// What a spawn takes from the base's storage.
pub const SPAWN_COST: (Item, u32) = (Item::Wood, 10);
/// What a base starts with.
pub const START: [(Item, u32); 4] = [
    (Item::Wood, 50),
    (Item::Wheat, 40),
    (Item::WheatSeed, 10),
    (Item::AppleSeed, 2),
];
/// How far a unit's own `say` reaches, without the radio network.
pub const SAY_RANGE: usize = 12;
/// Longest `say` or `mark`, in bytes.
pub const MESSAGE_BYTES: usize = 64;
/// A unit's health; it dies at 0.
pub const HP: u32 = 100;
/// Ticks a new unit goes before its first meal.
pub const FIRST_MEAL: u32 = 200;
/// Health a unit with nothing to eat loses each tick.
pub const STARVING: u32 = 1;
/// Farmland needs water within this range, or a well's.
pub const WATER_RANGE: usize = 4;
/// Fresh farmland's fertility, and the most it recovers to.
pub const FERTILE: u32 = 100;
/// Ticks of rest that recover one point of fertility.
pub const RECOVERY: u32 = 10;
/// What a hit with bare hands does.
pub const FIST: u32 = 3;
/// Hit points one `build` restores to a damaged building: less than a sword's hit, so
/// one repairer can't outlast an armed attacker.
pub const REPAIR: u32 = 5;
/// The vein tier bare hands gather.
pub const HAND_TIER: u32 = 1;
