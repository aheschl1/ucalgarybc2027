//! The generated terrain: smooth noise for height and moisture, thresholded into tiles,
//! and what is left to gather on each.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::content::Item;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

impl Coord {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// Chebyshev distance.
    pub fn dist(self, other: Coord) -> usize {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
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

/// One byte per tile in observations and snapshots: the discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Tile {
    Grass = 0,
    Forest = 1,
    Rock = 2,
    Water = 3,
    Dirt = 4,
    Coal = 5,
    Copper = 6,
    Iron = 7,
    Farmland = 8,
}

impl Tile {
    pub fn walkable(self) -> bool {
        matches!(
            self,
            Tile::Grass | Tile::Forest | Tile::Dirt | Tile::Farmland
        )
    }

    /// What gathering it gives, how much a fresh tile holds, and the tool tier it needs.
    fn resource(self) -> Option<(Item, u16, u32)> {
        RESOURCES
            .iter()
            .find(|r| r.0 == self)
            .map(|&(_, item, n, tier)| (item, n, tier))
    }
}

/// The tiles that give an item: the item, how much a fresh tile holds, and the tool tier
/// gathering it needs.
pub const RESOURCES: [(Tile, Item, u16, u32); 5] = [
    (Tile::Forest, Item::Wood, 20, 1),
    (Tile::Rock, Item::Stone, 30, 1),
    (Tile::Coal, Item::Coal, 40, 1),
    (Tile::Copper, Item::CopperOre, 40, 2),
    (Tile::Iron, Item::IronOre, 40, 3),
];

pub struct Map {
    size: usize,
    tiles: Vec<Tile>,
    /// What is left to gather on each tile.
    left: Vec<u16>,
}

/// A hash of a lattice point to [0, 1).
fn lattice(x: i64, y: i64, salt: u64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Value noise: `octaves` layers, each twice as fine and half as strong. In [0, 1).
fn noise(x: f64, y: f64, scale: f64, octaves: u32, salt: u64) -> f64 {
    let (mut total, mut weight, mut freq, mut amp) = (0.0, 0.0, 1.0 / scale, 1.0);
    for octave in 0..octaves {
        let (fx, fy) = (x * freq, y * freq);
        let (ix, iy) = (fx.floor() as i64, fy.floor() as i64);
        let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
        let (tx, ty) = (smooth(fx - ix as f64), smooth(fy - iy as f64));
        let salt = salt ^ (u64::from(octave) << 48);
        let top = lattice(ix, iy, salt) * (1.0 - tx) + lattice(ix + 1, iy, salt) * tx;
        let bottom = lattice(ix, iy + 1, salt) * (1.0 - tx) + lattice(ix + 1, iy + 1, salt) * tx;
        total += amp * (top * (1.0 - ty) + bottom * ty);
        weight += amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    total / weight
}

impl Map {
    /// Grass everywhere.
    pub fn flat(size: usize) -> Self {
        let tiles = vec![Tile::Grass; size * size];
        Self {
            size,
            left: vec![0; tiles.len()],
            tiles,
        }
    }

    /// Mirrored left to right, so both halves, and the teams on them, are alike.
    pub fn generate(size: usize, seed: u64) -> Self {
        let scale = size as f64 / 6.0;
        let mut tiles = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let (fx, fy) = (x.min(size - 1 - x) as f64, y as f64);
                let height = noise(fx, fy, scale, 5, seed);
                let wet = noise(fx, fy, scale, 4, seed ^ 0xA5A5);
                tiles.push(if height < 0.32 {
                    Tile::Water
                } else if height > 0.68 {
                    if noise(fx, fy, 6.0, 2, seed ^ 0x5A5A) > 0.72 {
                        // Which ore varies over wider areas than where veins are.
                        match noise(fx, fy, 40.0, 2, seed ^ 0x3C3C) {
                            n if n < 0.4 => Tile::Coal,
                            n if n < 0.65 => Tile::Copper,
                            _ => Tile::Iron,
                        }
                    } else {
                        Tile::Rock
                    }
                } else if wet > 0.55 {
                    Tile::Forest
                } else {
                    Tile::Grass
                });
            }
        }
        let left = tiles
            .iter()
            .map(|t| t.resource().map_or(0, |(_, n, _)| n))
            .collect();
        Self { size, tiles, left }
    }

    /// A square map from rows of `.` grass, `f` forest, `r` rock, `~` water, `c` coal,
    /// `u` copper, `i` iron.
    #[cfg(test)]
    pub fn from_text(text: &str) -> Self {
        let tiles: Vec<Tile> = text
            .split_whitespace()
            .flat_map(|row| row.chars())
            .map(|c| match c {
                '.' => Tile::Grass,
                'f' => Tile::Forest,
                'r' => Tile::Rock,
                '~' => Tile::Water,
                'c' => Tile::Coal,
                'u' => Tile::Copper,
                'i' => Tile::Iron,
                _ => panic!("unknown tile {c:?}"),
            })
            .collect();
        let size = tiles.len().isqrt();
        assert_eq!(size * size, tiles.len(), "not square");
        let left = tiles
            .iter()
            .map(|t| t.resource().map_or(0, |(_, n, _)| n))
            .collect();
        Self { size, tiles, left }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn tile(&self, at: Coord) -> Tile {
        self.tiles[at.y * self.size + at.x]
    }

    pub fn set(&mut self, at: Coord, tile: Tile) {
        self.tiles[at.y * self.size + at.x] = tile;
    }

    /// Whether water lies within `range` of `at`.
    pub fn water_near(&self, at: Coord, range: usize) -> bool {
        let last = self.size - 1;
        (at.y.saturating_sub(range)..=(at.y + range).min(last)).any(|y| {
            (at.x.saturating_sub(range)..=(at.x + range).min(last))
                .any(|x| self.tile(Coord::new(x, y)) == Tile::Water)
        })
    }

    /// The neighbouring tile, or None off the edge.
    pub fn step(&self, at: Coord, dir: Dir) -> Option<Coord> {
        let (x, y) = (at.x, at.y);
        let to = match dir {
            Dir::Up => Coord::new(x, y.checked_sub(1)?),
            Dir::Down => Coord::new(x, y + 1),
            Dir::Left => Coord::new(x.checked_sub(1)?, y),
            Dir::Right => Coord::new(x + 1, y),
        };
        (to.x < self.size && to.y < self.size).then_some(to)
    }

    /// What gathering the tile gives, and the tool tier it needs.
    pub fn resource(&self, at: Coord) -> Option<(Item, u32)> {
        self.tile(at).resource().map(|(item, _, tier)| (item, tier))
    }

    /// Takes one item from the tile. A tile gathered out turns to dirt.
    pub fn gather(&mut self, at: Coord) -> Option<Item> {
        let i = at.y * self.size + at.x;
        let (item, ..) = self.tiles[i].resource()?;
        self.left[i] -= 1;
        if self.left[i] == 0 {
            self.tiles[i] = Tile::Dirt;
        }
        Some(item)
    }

    /// Sets a tile and its mirror image, full of whatever it gives.
    fn paint(&mut self, x: usize, y: usize, tile: Tile) {
        for x in [x, self.size - 1 - x] {
            self.tiles[y * self.size + x] = tile;
            self.left[y * self.size + x] = tile.resource().map_or(0, |(_, n, _)| n);
        }
    }

    /// Readies the left half's base at `base`, and its mirror: clear ground around it,
    /// a forest grove to its west, rock to its south, and a pond to its north, whatever
    /// the noise made there.
    pub fn settle(&mut self, base: Coord) {
        let (x, y) = (base.x, base.y);
        for (xs, ys, tile) in [
            (x - 4..=x + 4, y - 4..=y + 4, Tile::Grass),
            (x - 9..=x - 5, y - 6..=y + 6, Tile::Forest),
            (x - 2..=x + 2, y + 5..=y + 7, Tile::Rock),
            (x - 1..=x + 1, y - 7..=y - 5, Tile::Water),
        ] {
            for y in ys {
                for x in xs.clone() {
                    self.paint(x, y, tile);
                }
            }
        }
    }

    /// The tile bytes of the rectangle at `origin`, row by row.
    pub fn window(&self, origin: Coord, width: usize, height: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(width * height);
        for y in origin.y..origin.y + height {
            let row = &self.tiles[y * self.size + origin.x..][..width];
            out.extend(row.iter().map(|&t| t as u8));
        }
        out
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.tiles.iter().map(|&t| t as u8).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_mixed() {
        let a = Map::generate(256, 7);
        assert_eq!(a.bytes(), Map::generate(256, 7).bytes());
        assert_ne!(a.bytes(), Map::generate(256, 8).bytes());
        let count = |t: Tile| a.tiles.iter().filter(|&&x| x == t).count();
        for t in [Tile::Grass, Tile::Forest, Tile::Rock, Tile::Water] {
            assert!(count(t) > 256 * 256 / 50, "{t:?}: {}", count(t));
        }
        assert!(count(Tile::Coal) + count(Tile::Copper) + count(Tile::Iron) > 0);
    }

    #[test]
    fn the_halves_mirror() {
        let map = Map::generate(64, 3);
        for y in 0..64 {
            for x in 0..32 {
                assert_eq!(map.tile(Coord::new(x, y)), map.tile(Coord::new(63 - x, y)));
            }
        }
    }

    /// From beside the base, a walk reaches a tile next to forest and one next to rock.
    #[test]
    fn the_starting_area_can_be_reached_on_any_map() {
        for seed in 0..20 {
            let mut map = Map::generate(64, seed);
            let base = Coord::new(16, 32);
            map.settle(base);
            let mut seen = vec![false; 64 * 64];
            let mut queue = vec![Coord::new(16, 33)];
            let (mut forest, mut rock) = (false, false);
            while let Some(at) = queue.pop() {
                for dir in Dir::ALL {
                    let Some(next) = map.step(at, dir) else {
                        continue;
                    };
                    forest |= map.tile(next) == Tile::Forest;
                    rock |= map.tile(next) == Tile::Rock;
                    let i = next.y * 64 + next.x;
                    if map.tile(next).walkable() && next != base && !seen[i] {
                        seen[i] = true;
                        queue.push(next);
                    }
                }
            }
            assert!(forest && rock, "seed {seed}");
            assert!(map.water_near(Coord::new(16, 28), 2), "seed {seed}");
        }
    }

    #[test]
    fn gathering_runs_a_tile_out_to_dirt() {
        let mut map = Map::generate(64, 1);
        let at = (0..64 * 64)
            .map(|i| Coord::new(i % 64, i / 64))
            .find(|&at| map.tile(at) == Tile::Forest)
            .unwrap();
        for _ in 0..20 {
            assert_eq!(map.gather(at), Some(Item::Wood));
        }
        assert_eq!(map.tile(at), Tile::Dirt);
        assert_eq!(map.gather(at), None);
    }

    #[test]
    fn windows_and_steps_stay_on_the_map() {
        let map = Map::generate(64, 1);
        assert_eq!(map.step(Coord::new(0, 0), Dir::Left), None);
        assert_eq!(map.step(Coord::new(63, 5), Dir::Right), None);
        assert_eq!(map.step(Coord::new(3, 5), Dir::Up), Some(Coord::new(3, 4)));
        let w = map.window(Coord::new(2, 3), 4, 2);
        assert_eq!(w.len(), 8);
        assert_eq!(w[5], map.tile(Coord::new(3, 4)) as u8);
    }
}
