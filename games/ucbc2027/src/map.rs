use prost::Message;
use schemars::JsonSchema;
use serde::Serialize;
use ucbc_engine::TeamId;

use crate::coord::Coord;
use crate::grid::Grid;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/ucbc2027.rs"));
}

const STANDARD: &[u8] = include_bytes!("../maps/standard.map");
pub const MAX_SIDE: u32 = 100;

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

/// Why a map file was refused.
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("not a map file: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("the board is {width}x{height}; each side must be 1 to {MAX_SIDE}")]
    Size { width: u32, height: u32 },
    #[error("a {width}x{height} board needs {} tiles, got {tiles}", width * height)]
    TileCount {
        width: u32,
        height: u32,
        tiles: usize,
    },
    #[error("tile {0} has unknown environment {1}")]
    UnknownEnvironment(Coord, i32),
    #[error("tile {0} has unknown item {1}")]
    UnknownItem(Coord, i32),
    #[error("the lab tile at {0} is not part of a 2x2 square of lab tiles")]
    Lab(Coord),
    #[error("a map needs exactly 2 labs, got {0}")]
    LabCount(usize),
    #[error("the fossil at {0} is not on an empty tile")]
    Fossil(Coord),
}

/// The board's environment and where the set starts.
pub struct Map {
    env: Grid<Environment>,
    /// Each lab's top-left tile, indexed by team.
    labs: [Coord; 2],
    fossils: Vec<Coord>,
}

impl Map {
    /// The map shipped with the game.
    pub fn standard() -> Self {
        Self::decode(STANDARD).expect("the standard map is valid")
    }

    /// Reads a map file, checking it describes a playable board.
    pub fn decode(bytes: &[u8]) -> Result<Self, MapError> {
        proto::Map::decode(bytes)?.try_into()
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

/// Checks the file describes a playable board.
impl TryFrom<proto::Map> for Map {
    type Error = MapError;

    fn try_from(file: proto::Map) -> Result<Self, MapError> {
        let proto::Map {
            width,
            height,
            tiles,
        } = file;
        if !(1..=MAX_SIDE).contains(&width) || !(1..=MAX_SIDE).contains(&height) {
            return Err(MapError::Size { width, height });
        }
        if tiles.len() != (width * height) as usize {
            return Err(MapError::TileCount {
                width,
                height,
                tiles: tiles.len(),
            });
        }

        let (width, height) = (width as usize, height as usize);
        let mut env = Grid::filled(width, height, Environment::Empty);
        let mut lab_tiles = Grid::filled(width, height, false);
        let mut fossils = Vec::new();
        for (i, tile) in tiles.into_iter().enumerate() {
            let c = Coord::new(i % width, i / width);
            let environment = proto::Environment::try_from(tile.environment)
                .map_err(|_| MapError::UnknownEnvironment(c, tile.environment))?;
            let item = proto::Item::try_from(tile.item)
                .map_err(|_| MapError::UnknownItem(c, tile.item))?;
            match environment {
                proto::Environment::Empty => {}
                proto::Environment::Wall => env[c] = Environment::Wall,
                proto::Environment::Lab => lab_tiles[c] = true,
            }
            match item {
                proto::Item::None => {}
                proto::Item::Fossil if environment == proto::Environment::Empty => fossils.push(c),
                proto::Item::Fossil => return Err(MapError::Fossil(c)),
            }
        }

        // In reading order, the first lab tile not yet in a lab is a new lab's top-left;
        // the lab is the 2x2 square from there, and the order gives its team.
        let mut labs = Vec::new();
        for (origin, _) in lab_tiles.iter().filter(|&(_, &lab)| lab) {
            if env[origin] != Environment::Empty {
                continue;
            }
            let team = labs.len() as u32;
            for c in lab_footprint(origin) {
                if lab_tiles.get(c) != Some(&true) || env[c] != Environment::Empty {
                    return Err(MapError::Lab(origin));
                }
                env[c] = Environment::Lab { team };
            }
            labs.push(origin);
        }
        let labs: [Coord; 2] = labs
            .try_into()
            .map_err(|labs: Vec<Coord>| MapError::LabCount(labs.len()))?;
        Ok(Self { env, labs, fossils })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map file drawn as rows: `.` empty, `#` wall, `L` lab, `f` a fossil on an empty tile.
    fn draw(rows: &[&str]) -> proto::Map {
        use proto::{Environment as E, Item as I};
        let tiles = rows
            .concat()
            .chars()
            .map(|ch| {
                let (environment, item) = match ch {
                    '.' => (E::Empty, I::None),
                    '#' => (E::Wall, I::None),
                    'L' => (E::Lab, I::None),
                    'f' => (E::Empty, I::Fossil),
                    _ => panic!("unknown tile {ch:?}"),
                };
                proto::Tile {
                    environment: environment as i32,
                    item: item as i32,
                }
            })
            .collect();
        proto::Map {
            width: rows[0].len() as u32,
            height: rows.len() as u32,
            tiles,
        }
    }

    const SMALL: [&str; 3] = ["LL..LL", "LL#.LL", "...f.."];

    fn refused(bytes: &[u8]) -> String {
        match Map::decode(bytes) {
            Ok(_) => panic!("expected the map to be refused"),
            Err(e) => e.to_string(),
        }
    }

    /// Every tile matches its `mirror` tile, with the teams' labs swapped.
    fn mirrors(map: &Map, mirror: impl Fn(Coord) -> Coord) -> bool {
        let swapped = |e| match e {
            Environment::Lab { team } => Environment::Lab { team: 1 - team },
            other => other,
        };
        map.env
            .iter()
            .all(|(c, &e)| map.env[mirror(c)] == swapped(e))
            && map
                .fossils
                .iter()
                .all(|&f| map.fossils.contains(&mirror(f)))
    }

    #[test]
    fn the_standard_map_is_mirrored() {
        let map = Map::standard();
        let (w, h) = (map.width(), map.height());
        assert!(
            mirrors(&map, |c| Coord::new(w - 1 - c.x, c.y))
                || mirrors(&map, |c| Coord::new(c.x, h - 1 - c.y))
        );
        assert_eq!((w, h), (16, 16));
        assert_eq!(map.fossils().len(), 6);
    }

    #[test]
    fn a_map_file_becomes_a_board() {
        let map = Map::decode(&draw(&SMALL).encode_to_vec()).unwrap();
        assert_eq!((map.width(), map.height()), (6, 3));
        assert_eq!(map.lab(TeamId(0)), Coord::new(0, 0));
        assert_eq!(map.lab(TeamId(1)), Coord::new(4, 0));
        for team in [0, 1] {
            for c in lab_footprint(map.lab(TeamId(team))) {
                assert_eq!(map.env(c), Some(Environment::Lab { team }));
            }
        }
        assert_eq!(map.env(Coord::new(2, 1)), Some(Environment::Wall));
        assert_eq!(map.env(Coord::new(3, 2)), Some(Environment::Empty));
        assert_eq!(map.fossils(), [Coord::new(3, 2)]);
    }

    #[test]
    fn labs_are_2x2_squares_numbered_in_reading_order() {
        let map = Map::decode(&draw(&["....", "..LL", "LLLL", "LL.."]).encode_to_vec()).unwrap();
        assert_eq!(map.lab(TeamId(0)), Coord::new(2, 1));
        assert_eq!(map.lab(TeamId(1)), Coord::new(0, 2));
        assert_eq!(
            map.env(Coord::new(3, 2)),
            Some(Environment::Lab { team: 0 })
        );
        assert_eq!(
            map.env(Coord::new(1, 3)),
            Some(Environment::Lab { team: 1 })
        );

        let cases: [(&[&str], &str); 5] = [
            (&["LL..LL", "L...LL"], "the lab tile at (0, 0) is not part"),
            (&["LL.LLL", "LL..LL"], "the lab tile at (3, 0) is not part"),
            (&["L.L.LL", "L.L.LL"], "the lab tile at (0, 0) is not part"),
            (&["LL....", "LL...."], "exactly 2 labs, got 1"),
            (&["LLLLLL", "LLLLLL"], "exactly 2 labs, got 3"),
        ];
        for (rows, why) in cases {
            let message = refused(&draw(rows).encode_to_vec());
            assert!(message.contains(why), "{message:?} should say {why:?}");
        }
    }

    #[test]
    fn a_map_file_must_have_a_size_and_known_tiles_and_fossils_on_empty_tiles() {
        type Break = fn(&mut proto::Map);
        let cases: Vec<(Break, &str)> = vec![
            (|m| m.width = 0, "each side must be 1 to 100"),
            (|m| m.height = 101, "each side must be 1 to 100"),
            (|m| m.tiles.truncate(17), "needs 18 tiles, got 17"),
            (
                |m| m.tiles[8].environment = 7,
                "(2, 1) has unknown environment 7",
            ),
            (|m| m.tiles[2].item = 7, "(2, 0) has unknown item 7"),
            (
                |m| m.tiles[8].item = 1,
                "the fossil at (2, 1) is not on an empty tile",
            ),
        ];
        for (break_it, why) in cases {
            let mut file = draw(&SMALL);
            break_it(&mut file);
            let message = refused(&file.encode_to_vec());
            assert!(message.contains(why), "{message:?} should say {why:?}");
        }
        assert!(refused(b"\xff\xff").starts_with("not a map file"));
    }
}
