//! The generated terrain: smooth noise for height and moisture, thresholded into tiles.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// One byte per tile in observations and snapshots: the discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Tile {
    Grass = 0,
    Forest = 1,
    Rock = 2,
    Water = 3,
    Ore = 4,
    Base = 5,
}

impl Tile {
    pub fn walkable(self) -> bool {
        matches!(self, Tile::Grass | Tile::Forest)
    }
}

pub struct Map {
    size: usize,
    tiles: Vec<Tile>,
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
        let salt = salt ^ u64::from(octave) << 48;
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
    pub fn generate(size: usize, seed: u64) -> Self {
        let scale = size as f64 / 6.0;
        let mut tiles = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let (fx, fy) = (x as f64, y as f64);
                let height = noise(fx, fy, scale, 5, seed);
                let wet = noise(fx, fy, scale, 4, seed ^ 0xA5A5);
                tiles.push(if height < 0.32 {
                    Tile::Water
                } else if height > 0.68 {
                    if noise(fx, fy, 6.0, 2, seed ^ 0x5A5A) > 0.72 {
                        Tile::Ore
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
        Self { size, tiles }
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

    /// Clears a square of grass `radius` out from `at`, so a base has room around it.
    pub fn clear(&mut self, at: Coord, radius: usize) {
        for y in at.y.saturating_sub(radius)..=(at.y + radius).min(self.size - 1) {
            for x in at.x.saturating_sub(radius)..=(at.x + radius).min(self.size - 1) {
                self.set(Coord::new(x, y), Tile::Grass);
            }
        }
    }

    /// The tile bytes of the rectangle at `origin`, row by row.
    pub fn window(&self, origin: Coord, width: usize, height: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(width * height);
        for y in origin.y..origin.y + height {
            let row = y * self.size;
            out.extend(
                self.tiles[row + origin.x..row + origin.x + width]
                    .iter()
                    .map(|&t| t as u8),
            );
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
        let a = Map::generate(128, 7);
        assert_eq!(a.bytes(), Map::generate(128, 7).bytes());
        assert_ne!(a.bytes(), Map::generate(128, 8).bytes());
        let count = |t: Tile| a.tiles.iter().filter(|&&x| x == t).count();
        for t in [Tile::Grass, Tile::Forest, Tile::Rock, Tile::Water] {
            assert!(count(t) > 128 * 128 / 50, "{t:?}: {}", count(t));
        }
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
