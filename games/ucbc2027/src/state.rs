use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{ActionError, BotId, BotManager, QueryError, TeamId};

use crate::{bot::BotType, game::Coord};

const WIDTH: usize = 16;
const HEIGHT: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Environment {
    Empty,
    Wall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Item {
    Bot { team: u32, bot_type: BotType },
}

impl Item {
    fn bot(bots: &BotManager<BotType>, id: BotId) -> Self {
        let bot = &bots[id];
        Item::Bot {
            team: bot.team().0,
            bot_type: **bot,
        }
    }
}

/// A square as bots and the replay see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Tile {
    pub environment: Environment,
    pub item: Option<Item>,
}

/// The snapshot: the tick and every tile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct State {
    pub tick: u32,
    pub board: Vec<Vec<Tile>>,
}

/// A square of the board. A bot is kept by id; its team and type are the game's
/// `BotManager`'s.
#[derive(Clone, Copy)]
struct Cell {
    environment: Environment,
    bot: Option<BotId>,
}

pub struct Board {
    pub tick: u32,
    cells: Vec<Vec<Cell>>,
    base_locations: Vec<Coord>,
}

impl Board {
    /// An empty board; team 0's base goes top-left and team 1's bottom-right.
    pub fn new() -> Self {
        let empty = Cell {
            environment: Environment::Empty,
            bot: None,
        };
        Self {
            tick: 0,
            cells: vec![vec![empty; WIDTH]; HEIGHT],
            base_locations: vec![Coord::new(0, 0), Coord::new(WIDTH - 1, HEIGHT - 1)],
        }
    }

    pub const fn width(&self) -> usize {
        WIDTH
    }

    pub const fn height(&self) -> usize {
        HEIGHT
    }

    pub fn base_location(&self, team: TeamId) -> Coord {
        // lowkey might fail, but i dont want to add the result rn
        self.base_locations[team.0 as usize]
    }

    pub fn tile(&self, at: &Coord, bots: &BotManager<BotType>) -> Result<Tile, QueryError> {
        let cell = self.cell(at).ok_or_else(|| {
            QueryError::Rejected(format!("({}, {}) is off the board", at.x, at.y))
        })?;
        Ok(Self::view(cell, bots))
    }

    pub fn snapshot(&self, bots: &BotManager<BotType>) -> State {
        State {
            tick: self.tick,
            board: self
                .cells
                .iter()
                .map(|row| row.iter().map(|cell| Self::view(cell, bots)).collect())
                .collect(),
        }
    }

    /// Whether `team`'s base may spawn a bot at `at`: next to the base, on open
    /// ground, and free.
    pub fn check_spawn(&self, at: &Coord, team: TeamId) -> Result<(), ActionError> {
        let cell = self
            .cell(at)
            .ok_or_else(|| ActionError::Invalid("out of bounds".into()))?;
        if !self.base_location(team).around().any(|c| c == *at) {
            return Err(ActionError::Invalid("must spawn next to the base".into()));
        }
        if cell.environment != Environment::Empty {
            return Err(ActionError::Invalid("cannot spawn on a wall".into()));
        }
        if cell.bot.is_some() {
            return Err(ActionError::Invalid("a bot is already there".into()));
        }
        Ok(())
    }

    pub fn place(&mut self, at: &Coord, bot: BotId) {
        self.cells[at.y][at.x].bot = Some(bot);
    }

    pub fn remove(&mut self, bot: BotId) {
        for cell in self.cells.iter_mut().flatten() {
            if cell.bot == Some(bot) {
                cell.bot = None;
            }
        }
    }

    fn cell(&self, at: &Coord) -> Option<&Cell> {
        self.cells.get(at.y)?.get(at.x)
    }

    fn view(cell: &Cell, bots: &BotManager<BotType>) -> Tile {
        Tile {
            environment: cell.environment,
            item: cell.bot.map(|id| Item::bot(bots, id)),
        }
    }
}
