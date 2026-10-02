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
}

/// A team's base. Its tiles are environment; the lab bot spawns dinos.
pub struct Lab {
    pub origin: Coord,
    pub health: u32,
}

impl Lab {
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
    /// A fresh level 1 dino.
    pub fn new(pos: Coord) -> Self {
        Self {
            pos,
            level: 1,
            health: rules::DINO_HEALTH,
            held: None,
        }
    }

    /// The same for every dino until levels and artifacts change it.
    pub fn stats(&self) -> Stats {
        Stats {
            move_range: rules::MOVE_RANGE,
            action_range: rules::ACTION_RANGE,
        }
    }
}

/// What a dino can reach.
pub struct Stats {
    /// How far one move may go.
    pub move_range: usize,
    /// How far away a grab or drop may reach.
    pub action_range: usize,
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
}
