//! What leaves the game: the `view` reply and the replay snapshot.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::map::Coord;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Pacman,
    Ghost,
}

/// Pac-Man or a ghost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentView {
    pub id: u64,
    pub role: Role,
    pub pos: Coord,
    /// Ticks this ghost stays frightened; 0 if it isn't, and always 0 for Pac-Man.
    /// A frightened ghost moves only on even ticks, and Pac-Man eats it by touching it.
    pub frightened: u32,
}

/// The whole board, as a bot sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct View {
    pub tick: u32,
    pub me: AgentView,
    /// Rows of tiles, `maze[y][x]`: `#` wall, `.` pellet, `o` power pellet, ` ` empty,
    /// `G` ghost house (ghosts only). Moving off an edge wraps to the other side.
    pub maze: Vec<String>,
    pub pacman: AgentView,
    pub ghosts: Vec<AgentView>,
    /// Pellets and power pellets left; Pac-Man wins when it reaches 0.
    pub pellets_left: u32,
}

/// The whole set after a tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    pub tick: u32,
    /// Rows of tiles, as in [`View::maze`].
    pub maze: Vec<String>,
    /// Live agents in step order, Pac-Man first.
    pub agents: Vec<AgentView>,
    pub pacman_team: u32,
    pub ghost_team: u32,
    pub pellets_left: u32,
    pub ghosts_eaten: u32,
}
