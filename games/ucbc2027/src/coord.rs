use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A tile: `x` grows to the right, `y` downward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

impl Coord {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// Chebyshev distance: a diagonal neighbour is one away.
    pub fn dist(self, other: Coord) -> usize {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonals_are_one_away() {
        let c = Coord::new(3, 3);
        assert_eq!(c.dist(Coord::new(4, 4)), 1);
        assert_eq!(c.dist(Coord::new(2, 4)), 1);
        assert_eq!(c.dist(Coord::new(3, 3)), 0);
        assert_eq!(c.dist(Coord::new(0, 5)), 3);
    }
}
