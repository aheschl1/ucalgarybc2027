use crate::coord::Coord;
use crate::map::lab_footprint;
use crate::rules;
use crate::state::Item;

/// What the game keeps for each bot.
pub enum Unit {
    Lab(Lab),
    Dino(Dino),
}

impl Unit {
    pub fn as_lab(&self) -> Option<&Lab> {
        match self {
            Unit::Lab(lab) => Some(lab),
            Unit::Dino(_) => None,
        }
    }

    pub fn as_dino(&self) -> Option<&Dino> {
        match self {
            Unit::Dino(dino) => Some(dino),
            Unit::Lab(_) => None,
        }
    }

    pub fn as_dino_mut(&mut self) -> Option<&mut Dino> {
        match self {
            Unit::Dino(dino) => Some(dino),
            Unit::Lab(_) => None,
        }
    }

    /// How far it can see.
    pub fn vision(&self) -> usize {
        match self {
            Unit::Lab(_) => Lab::VISION,
            Unit::Dino(dino) => dino.stats().vision,
        }
    }

    /// Whether `at` is within its vision
    pub fn sees(&self, at: Coord) -> bool {
        let dist = match self {
            Unit::Lab(lab) => lab_footprint(lab.origin)
                .into_iter()
                .map(|c| c.dist(at))
                .min()
                .expect("a lab has tiles"),
            Unit::Dino(dino) => dino.pos.dist(at),
        };
        dist <= self.vision()
    }
}

/// A team's base. Its tiles are environment; the lab bot spawns dinos.
pub struct Lab {
    pub origin: Coord,
    pub health: u32,
}

impl Lab {
    pub const VISION: usize = 2;

    pub fn new(origin: Coord) -> Self {
        Self {
            origin,
            health: rules::LAB_HEALTH,
        }
    }

    /// Next to the lab, diagonals included, and not on it: where a spawn may land.
    pub fn borders(&self, at: Coord) -> bool {
        let footprint = lab_footprint(self.origin);
        !footprint.contains(&at) && footprint.iter().any(|c| c.dist(at) == 1)
    }
}

pub struct Dino {
    pub pos: Coord,
    pub level: u32,
    pub health: u32,
    /// At most one thing at a time.
    pub held: Option<Item>,
}

impl Dino {
    pub const VISION: usize = 3;

    /// A fresh level 1 dino.
    pub fn new(pos: Coord) -> Self {
        Self {
            pos,
            level: 1,
            health: rules::DINO_HEALTH,
            held: None,
        }
    }

    pub fn stats(&self) -> Stats {
        Stats {
            move_range: rules::MOVE_RANGE,
            action_range: rules::ACTION_RANGE,
            attack_range: rules::ATTACK_RANGE,
            vision: Self::VISION + (self.level as usize - 1) / 2,
        }
    }
}

/// What a dino can reach.
pub struct Stats {
    /// How far one move may go.
    pub move_range: usize,
    /// How far away a grab or drop may reach.
    pub action_range: usize,
    /// How far away an attack may reach.
    pub attack_range: usize,
    /// How far away a tile may be queried.
    pub vision: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lab_borders_the_ring_around_its_footprint() {
        let lab = Lab::new(Coord::new(1, 1));
        let ring: Vec<Coord> = (0..5)
            .flat_map(|y| (0..5).map(move |x| Coord::new(x, y)))
            .filter(|&c| lab.borders(c))
            .collect();
        assert_eq!(ring.len(), 12);
        assert!(lab.borders(Coord::new(0, 0)));
        assert!(lab.borders(Coord::new(3, 3)));
        assert!(!lab.borders(Coord::new(2, 2)));
        assert!(!lab.borders(Coord::new(4, 1)));
    }

    #[test]
    fn a_lab_sees_from_every_tile_of_its_footprint() {
        let lab = Unit::Lab(Lab::new(Coord::new(3, 3)));
        assert!(lab.sees(Coord::new(1, 1)));
        assert!(lab.sees(Coord::new(6, 6)));
        assert!(!lab.sees(Coord::new(0, 4)));
        assert!(!lab.sees(Coord::new(7, 4)));
    }

    #[test]
    fn dino_vision_grows_every_two_levels() {
        let mut dino = Dino::new(Coord::new(0, 0));
        for (level, vision) in [(1, 3), (2, 3), (3, 4), (4, 4), (5, 5)] {
            dino.level = level;
            assert_eq!(dino.stats().vision, vision, "level {level}");
        }
        let dino = Unit::Dino(Dino::new(Coord::new(5, 5)));
        assert!(dino.sees(Coord::new(8, 2)));
        assert!(!dino.sees(Coord::new(9, 5)));
    }

    /// Nothing can act on a tile it cannot see.
    #[test]
    fn vision_covers_every_reach() {
        let stats = Dino::new(Coord::new(0, 0)).stats();
        let reach = [stats.move_range, stats.action_range, stats.attack_range];
        assert!(reach.iter().all(|&r| r <= stats.vision));
        const { assert!(Lab::VISION >= 1, "a lab must see its spawn ring") };
    }
}
