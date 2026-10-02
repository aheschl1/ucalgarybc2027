use ucbc_engine::{BotId, TeamId};

use crate::coord::Coord;
use crate::grid::Grid;
use crate::map::Map;
use crate::rules;
use crate::unit::Dino;

/// The one movable thing a tile can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Dino(BotId),
    Fossil,
}

pub struct TeamState {
    pub bones: u32,
}

impl TeamState {
    pub fn new() -> Self {
        Self {
            bones: rules::START_BONES,
        }
    }
}

/// Everything about a set except the bots' own data, which is in the `BotManager`.
pub struct State {
    pub tick: u32,
    pub map: Map,
    pub items: Grid<Option<Item>>,
    /// Indexed by team.
    pub teams: Vec<TeamState>,
}

impl State {
    /// The map's fossils laid out, and a fresh team for each of its labs.
    pub fn new(map: Map) -> Self {
        let mut items = Grid::filled(map.width(), map.height(), None);
        for &at in map.fossils() {
            items[at] = Some(Item::Fossil);
        }
        let teams = (0..map.teams()).map(|_| TeamState::new()).collect();
        Self {
            tick: 0,
            map,
            items,
            teams,
        }
    }

    /// On the board, walkable, and nothing on it.
    pub fn is_free(&self, at: Coord) -> bool {
        self.map.walkable(at) && self.items.get(at) == Some(&None)
    }

    /// Clears the dino's tile, leaving whatever it held there. The caller removes
    /// the bot itself.
    pub fn clear_dino(&mut self, dino: &Dino) {
        self.items[dino.pos] = dino.held;
    }

    pub fn team(&self, team: TeamId) -> &TeamState {
        &self.teams[team.0 as usize]
    }

    pub fn team_mut(&mut self, team: TeamId) -> &mut TeamState {
        &mut self.teams[team.0 as usize]
    }
}
