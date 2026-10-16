//! The numbers to tune.

use crate::content::Item;

/// Items a unit carries, of any kind together.
pub const CARRY: u32 = 20;
/// What a spawn takes from the base's storage.
pub const SPAWN_COST: (Item, u32) = (Item::Wood, 10);
/// What a base starts with.
pub const START: (Item, u32) = (Item::Wood, 50);
/// How far a unit's own `say` reaches, without the radio network.
pub const SAY_RANGE: usize = 12;
/// Longest `say` or `mark`, in bytes.
pub const MESSAGE_BYTES: usize = 64;
