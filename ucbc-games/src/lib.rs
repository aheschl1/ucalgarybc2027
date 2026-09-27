//! The one list of games. A game is a crate under `games/`, joined here by a path
//! dependency, a feature, and a line in [`registry`].

use ucbc_engine::GameRegistry;

/// Every game compiled in.
pub fn registry() -> GameRegistry {
    // `mut` is unused in a build with no game features.
    #[allow(unused_mut)]
    let mut registry = GameRegistry::new();
    #[cfg(feature = "tictactoe")]
    registry.register::<ucbc_tictactoe::TicTacToe>();
    #[cfg(feature = "ucbc2027")]
    registry.register::<ucbc_2027::Ucbc2027>();
    registry
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_game_describes_its_api() {
        let registry = super::registry();
        for name in registry.names() {
            assert_eq!(registry.api(name).unwrap().name, name);
        }
    }
}
