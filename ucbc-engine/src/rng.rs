//! Seed derivation. ChaCha8 is a fixed algorithm, so replays stay reproducible across
//! crate upgrades.

use rand::{Rng as _, SeedableRng};

use crate::ids::{BotId, TeamId};

pub use rand_chacha::ChaCha8Rng;

pub fn set_seed(match_seed: u64, set_index: u32) -> u64 {
    let mut rng = ChaCha8Rng::seed_from_u64(match_seed);
    rng.set_stream(u64::from(set_index));
    rng.next_u64()
}

pub fn bot_seed(set_seed: u64, team: TeamId, bot: BotId) -> u64 {
    let mut rng = ChaCha8Rng::seed_from_u64(set_seed);
    rng.set_stream((u64::from(team.0) << 48) ^ bot.0);
    rng.next_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_are_deterministic_and_distinct() {
        assert_eq!(set_seed(1, 0), set_seed(1, 0));
        assert_ne!(set_seed(1, 0), set_seed(1, 1));
        assert_ne!(set_seed(1, 0), set_seed(2, 0));
        let s = set_seed(1, 0);
        assert_ne!(
            bot_seed(s, TeamId(0), BotId(0)),
            bot_seed(s, TeamId(0), BotId(1))
        );
        assert_ne!(
            bot_seed(s, TeamId(0), BotId(0)),
            bot_seed(s, TeamId(1), BotId(0))
        );
    }
}
