//! The maze, read from text: one character per tile, one line per row.
//!
//! ```text
//! #  wall
//! .  pellet
//! o  power pellet
//!    (space) empty floor
//! P  Pac-Man's start
//! G  ghost house: a ghost starts on each, and Pac-Man can't enter
//! ```
//!
//! Moving off an edge wraps to the opposite one, which is how tunnels work.

use std::collections::VecDeque;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::rules::MAX_SIDE;

/// A tile: `x` grows to the right, `y` downward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

impl Coord {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Empty,
    Pellet,
    Power,
    House,
}

impl Tile {
    pub fn symbol(self) -> char {
        match self {
            Tile::Wall => '#',
            Tile::Empty => ' ',
            Tile::Pellet => '.',
            Tile::Power => 'o',
            Tile::House => 'G',
        }
    }

    pub fn is_food(self) -> bool {
        matches!(self, Tile::Pellet | Tile::Power)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MapError(pub String);

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Map {
    width: usize,
    height: usize,
    /// Row by row, `tiles[y * width + x]`.
    tiles: Vec<Tile>,
    pub pacman: Coord,
    /// In reading order: the order the ghosts spawn and step.
    pub ghosts: Vec<Coord>,
}

impl Map {
    pub fn standard() -> Self {
        Self::parse(include_str!("../maps/standard.map")).expect("the standard map is valid")
    }

    pub fn parse(text: &str) -> Result<Self, MapError> {
        let bad = |why: String| Err(MapError(why));
        let rows: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let height = rows.len();
        let width = rows.first().map_or(0, |r| r.chars().count());
        if width == 0 || height == 0 {
            return bad("the map is empty".into());
        }
        if width > MAX_SIDE || height > MAX_SIDE {
            return bad(format!("{width}x{height} is over {MAX_SIDE} on a side"));
        }
        let mut tiles = Vec::with_capacity(width * height);
        let mut pacman = Vec::new();
        let mut ghosts = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            if row.chars().count() != width {
                return bad(format!(
                    "row {y} is {} wide, the first is {width}",
                    row.chars().count()
                ));
            }
            for (x, c) in row.chars().enumerate() {
                tiles.push(match c {
                    '#' => Tile::Wall,
                    ' ' => Tile::Empty,
                    '.' => Tile::Pellet,
                    'o' => Tile::Power,
                    'P' => {
                        pacman.push(Coord::new(x, y));
                        Tile::Empty
                    }
                    'G' => {
                        ghosts.push(Coord::new(x, y));
                        Tile::House
                    }
                    _ => return bad(format!("unknown tile {c:?} at ({x}, {y})")),
                });
            }
        }
        let &[pacman] = pacman.as_slice() else {
            return bad(format!("needs exactly one P, found {}", pacman.len()));
        };
        if ghosts.is_empty() {
            return bad("needs at least one G".into());
        }
        let map = Self {
            width,
            height,
            tiles,
            pacman,
            ghosts,
        };
        map.check_reachable()?;
        Ok(map)
    }

    /// Every pellet is reachable by Pac-Man, and Pac-Man by every ghost.
    fn check_reachable(&self) -> Result<(), MapError> {
        let pacman = self.reachable(self.pacman, |t| t != Tile::House);
        if let Some(at) = self
            .coords()
            .find(|&at| self.tiles[self.index(at)].is_food() && !pacman[self.index(at)])
        {
            return Err(MapError(format!("Pac-Man can't reach the pellet at {at}")));
        }
        if self.pellets() == 0 {
            return Err(MapError("needs at least one pellet".into()));
        }
        for &ghost in &self.ghosts {
            if !self.reachable(ghost, |_| true)[self.index(self.pacman)] {
                return Err(MapError(format!(
                    "the ghost at {ghost} can't reach Pac-Man"
                )));
            }
        }
        Ok(())
    }

    /// Which tiles a walk from `from` reaches, through tiles that aren't walls and pass
    /// `open`.
    fn reachable(&self, from: Coord, open: impl Fn(Tile) -> bool) -> Vec<bool> {
        let mut seen = vec![false; self.tiles.len()];
        seen[self.index(from)] = true;
        let mut queue = VecDeque::from([from]);
        while let Some(at) = queue.pop_front() {
            for dir in Dir::ALL {
                let next = self.step(at, dir);
                let tile = self.tile(next);
                if tile != Tile::Wall && open(tile) && !seen[self.index(next)] {
                    seen[self.index(next)] = true;
                    queue.push_back(next);
                }
            }
        }
        seen
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    fn index(&self, at: Coord) -> usize {
        at.y * self.width + at.x
    }

    fn coords(&self) -> impl Iterator<Item = Coord> + '_ {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| Coord::new(x, y)))
    }

    pub fn tile(&self, at: Coord) -> Tile {
        self.tiles[self.index(at)]
    }

    pub fn set(&mut self, at: Coord, tile: Tile) {
        let i = self.index(at);
        self.tiles[i] = tile;
    }

    /// The neighbouring tile, wrapping around the edges.
    pub fn step(&self, at: Coord, dir: Dir) -> Coord {
        let (w, h) = (self.width, self.height);
        match dir {
            Dir::Up => Coord::new(at.x, (at.y + h - 1) % h),
            Dir::Down => Coord::new(at.x, (at.y + 1) % h),
            Dir::Left => Coord::new((at.x + w - 1) % w, at.y),
            Dir::Right => Coord::new((at.x + 1) % w, at.y),
        }
    }

    /// Pellets and power pellets left.
    pub fn pellets(&self) -> u32 {
        self.tiles.iter().filter(|t| t.is_food()).count() as u32
    }

    /// One string per row, in the map's own symbols, without the starts.
    pub fn rows(&self) -> Vec<String> {
        self.tiles
            .chunks(self.width)
            .map(|row| row.iter().map(|t| t.symbol()).collect())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_map_has_four_ghosts() {
        let map = Map::standard();
        assert_eq!(map.ghosts.len(), 4);
        assert_eq!((map.width(), map.height()), (19, 21));
        assert!(map.pellets() > 100);
    }

    #[test]
    fn edges_wrap() {
        let map = Map::parse("P.G\n...").unwrap();
        assert_eq!(map.step(Coord::new(0, 0), Dir::Left), Coord::new(2, 0));
        assert_eq!(map.step(Coord::new(0, 0), Dir::Up), Coord::new(0, 1));
        assert_eq!(map.step(Coord::new(2, 1), Dir::Right), Coord::new(0, 1));
    }

    #[test]
    fn rows_show_the_tiles_without_pacman() {
        let map = Map::parse("#P.oG#").unwrap();
        assert_eq!(map.rows(), vec!["# .oG#"]);
    }

    #[test]
    fn rejects_bad_maps() {
        let err = |text: &str| Map::parse(text).unwrap_err().0;
        assert!(err("").contains("empty"));
        assert!(err("P.G\n..").contains("row 1"));
        assert!(err("P.x G").contains("unknown tile"));
        assert!(err("..G").contains("exactly one P"));
        assert!(err("P..").contains("at least one G"));
        assert!(err("P G").contains("at least one pellet"));
        // The pellet is behind the ghost house.
        assert!(err("#####\n#PG.#\n#####").contains("can't reach the pellet"));
        assert!(err("#####\n#P.##\n###G#").contains("can't reach Pac-Man"));
    }
}
