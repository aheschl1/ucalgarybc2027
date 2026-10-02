use schemars::JsonSchema;
use serde::Serialize;
use ucbc_engine::TeamId;

use crate::coord::Coord;
use crate::grid::Grid;

const WIDTH: usize = 16;
const HEIGHT: usize = 16;

/// What a tile is made of. Fixed for the whole set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Environment {
    Empty,
    Wall,
    /// One of a team's four lab tiles.
    Lab {
        team: u32,
    },
}

/// The tiles a lab with top-left corner `origin` covers.
pub fn lab_footprint(origin: Coord) -> [Coord; 4] {
    let Coord { x, y } = origin;
    [
        Coord::new(x, y),
        Coord::new(x + 1, y),
        Coord::new(x, y + 1),
        Coord::new(x + 1, y + 1),
    ]
}

/// The board's environment and where the set starts.
pub struct Map {
    env: Grid<Environment>,
    /// Each lab's top-left tile, indexed by team.
    labs: [Coord; 2],
    fossils: Vec<Coord>,
}

impl Map {
    /// The one map so far: team 0's lab on the left, team 1's mirrored on the right.
    pub fn standard() -> Self {
        let mirror = |c: Coord| Coord::new(WIDTH - 1 - c.x, c.y);
        let walls = [(5, 3), (5, 4), (5, 5), (5, 10), (5, 11), (5, 12)];
        let fossils = [(3, 2), (3, 13), (6, 7)];

        let mut env = Grid::filled(WIDTH, HEIGHT, Environment::Empty);
        for (x, y) in walls {
            let c = Coord::new(x, y);
            env[c] = Environment::Wall;
            env[mirror(c)] = Environment::Wall;
        }
        // The left lab's top-left mirrors to the right lab's top-right.
        let left = Coord::new(1, 7);
        let right = Coord::new(mirror(left).x - 1, left.y);
        let labs = [left, right];
        for (team, origin) in labs.into_iter().enumerate() {
            for c in lab_footprint(origin) {
                env[c] = Environment::Lab { team: team as u32 };
            }
        }
        let fossils = fossils
            .into_iter()
            .map(|(x, y)| Coord::new(x, y))
            .flat_map(|c| [c, mirror(c)])
            .collect();
        Self { env, labs, fossils }
    }

    pub fn width(&self) -> usize {
        self.env.width()
    }

    pub fn height(&self) -> usize {
        self.env.height()
    }

    /// `None` off the board.
    pub fn env(&self, at: Coord) -> Option<Environment> {
        self.env.get(at).copied()
    }

    /// A dino may stand here.
    pub fn walkable(&self, at: Coord) -> bool {
        self.env(at) == Some(Environment::Empty)
    }

    /// How many teams the map has a lab for.
    pub fn teams(&self) -> usize {
        self.labs.len()
    }

    /// The top-left tile of `team`'s lab.
    pub fn lab(&self, team: TeamId) -> Coord {
        self.labs[team.0 as usize]
    }

    /// Where fossils lie at the start of a set.
    pub fn fossils(&self) -> &[Coord] {
        &self.fossils
    }

    pub fn rows(&self) -> impl Iterator<Item = &[Environment]> {
        self.env.rows()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_map_is_mirrored() {
        let map = Map::standard();
        for (c, &e) in map.env.iter() {
            let m = map.env[Coord::new(WIDTH - 1 - c.x, c.y)];
            let expected = match e {
                Environment::Lab { team } => Environment::Lab { team: 1 - team },
                other => other,
            };
            assert_eq!(m, expected, "at {c:?}");
        }
        for &f in map.fossils() {
            assert!(map.walkable(f));
            assert!(map.fossils().contains(&Coord::new(WIDTH - 1 - f.x, f.y)));
        }
    }

    #[test]
    fn each_lab_covers_its_footprint() {
        let map = Map::standard();
        for team in [0, 1] {
            for c in lab_footprint(map.lab(TeamId(team))) {
                assert_eq!(map.env(c), Some(Environment::Lab { team }));
            }
        }
        let labs = map
            .env
            .iter()
            .filter(|(_, e)| matches!(e, Environment::Lab { .. }))
            .count();
        assert_eq!(labs, 8);
    }
}
