use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, Outcome,
    QueryError, Request, SetSetup, TeamId,
};

use crate::map::{Coord, Dir, Map, Tile};
use crate::rules::FRIGHTENED_TICKS;
use crate::view::{AgentView, Role, Snapshot, View};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    /// The whole board: the maze, every agent, and which one is you.
    View(Request<(), View>),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Move one tile, wrapping at the edges; returns where you are now. Refused into a
    /// wall, into the ghost house for Pac-Man, a second time in a step, and on odd ticks
    /// for a frightened ghost. Not moving is allowed: you stay where you are.
    Move(Request<Heading, Coord>),
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
pub struct Heading {
    pub dir: Dir,
}

pub struct Agent {
    role: Role,
    pos: Coord,
    /// Where an eaten ghost goes back to.
    spawn: Coord,
    frightened: u32,
}

/// Two teams: the set's first team is Pac-Man, the other has a ghost for each `G` on
/// the map. Pac-Man steps first each tick, then the ghosts in map order.
pub struct Pacman {
    bots: BotManager<Agent>,
    map: Map,
    tick: u32,
    pacman: BotId,
    pacman_team: TeamId,
    ghost_team: TeamId,
    /// Whether the stepping bot has moved this step.
    moved: bool,
    ghosts_eaten: u32,
    outcome: Option<Outcome>,
}

fn invalid(message: &str) -> ActionError {
    ActionError::Invalid(message.into())
}

/// The set's map from `game_config`, `{"maps": ["<a map file in base64>", ...]}`: one map
/// plays every set, otherwise set `i` plays `maps[i]`. Without `maps`, the standard one.
fn map_from(config: Option<&Value>, set_index: u32) -> Result<Map, EngineError> {
    let Some(maps) = config.and_then(|c| c.get("maps")) else {
        return Ok(Map::standard());
    };
    let bad = |why: String| EngineError::Config(format!("game_config.maps: {why}"));
    let maps = maps
        .as_array()
        .ok_or_else(|| bad("expected a list of base64 strings".into()))?;
    let i = if maps.len() == 1 {
        0
    } else {
        set_index as usize
    };
    let map = maps.get(i).ok_or_else(|| {
        bad(format!(
            "set {set_index} has no map; give 1 map or one per set, got {}",
            maps.len()
        ))
    })?;
    let bad = |why: String| EngineError::Config(format!("game_config.maps[{i}]: {why}"));
    let bytes = map
        .as_str()
        .ok_or_else(|| bad("expected a base64 string".into()))
        .and_then(|s| BASE64.decode(s).map_err(|e| bad(e.to_string())))?;
    let text = String::from_utf8(bytes).map_err(|e| bad(e.to_string()))?;
    Map::parse(&text).map_err(|e| bad(e.to_string()))
}

impl Pacman {
    fn agent_view(&self, id: BotId) -> AgentView {
        let agent = &self.bots[id];
        AgentView {
            id: id.0,
            role: agent.role,
            pos: agent.pos,
            frightened: agent.frightened,
        }
    }

    fn ghosts(&self) -> impl Iterator<Item = BotId> + '_ {
        self.bots.ids().into_iter().filter(|&id| id != self.pacman)
    }

    fn end(&mut self, outcome: Outcome) {
        if self.outcome.is_none() {
            self.outcome = Some(outcome);
        }
    }

    fn move_agent(&mut self, bot: BotId, dir: Dir) -> Result<Coord, ActionError> {
        if self.moved {
            return Err(invalid("already moved this step"));
        }
        let agent = &self.bots[bot];
        if agent.frightened > 0 && !self.tick.is_multiple_of(2) {
            return Err(invalid("a frightened ghost moves only on even ticks"));
        }
        let to = self.map.step(agent.pos, dir);
        match self.map.tile(to) {
            Tile::Wall => return Err(invalid("that's a wall")),
            Tile::House if agent.role == Role::Pacman => {
                return Err(invalid("Pac-Man can't enter the ghost house"));
            }
            _ => {}
        }
        self.bots[bot].pos = to;
        self.moved = true;
        if bot == self.pacman {
            self.eat(to);
        }
        self.collide();
        if self.map.pellets() == 0 {
            self.end(Outcome::win(self.pacman_team).with_detail("Pac-Man cleared the maze"));
        }
        Ok(to)
    }

    fn eat(&mut self, at: Coord) {
        match self.map.tile(at) {
            Tile::Pellet => self.map.set(at, Tile::Empty),
            Tile::Power => {
                self.map.set(at, Tile::Empty);
                for id in self.ghosts().collect::<Vec<_>>() {
                    self.bots[id].frightened = FRIGHTENED_TICKS;
                }
            }
            _ => {}
        }
    }

    /// Pac-Man and the ghosts on its tile: it eats the frightened ones, and any other
    /// catches it.
    fn collide(&mut self) {
        let at = self.bots[self.pacman].pos;
        for id in self.ghosts().collect::<Vec<_>>() {
            let ghost = &mut self.bots[id];
            if ghost.pos != at {
                continue;
            }
            if ghost.frightened > 0 {
                ghost.pos = ghost.spawn;
                ghost.frightened = 0;
                self.ghosts_eaten += 1;
            } else {
                self.end(Outcome::win(self.ghost_team).with_detail("the ghosts caught Pac-Man"));
            }
        }
    }
}

impl Game for Pacman {
    const NAME: &'static str = "pacman";
    type Query = Queries;
    type Action = Action;
    type Snapshot = Snapshot;
    type Bot = Agent;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        let &[pacman_team, ghost_team] = setup.teams.as_slice() else {
            return Err(EngineError::Config(format!(
                "pacman needs exactly 2 teams, got {}",
                setup.teams.len()
            )));
        };
        let map = map_from(setup.game_config.as_ref(), setup.set_index)?;
        let mut bots = BotManager::new();
        let pacman = bots.spawn(
            pacman_team,
            Agent {
                role: Role::Pacman,
                pos: map.pacman,
                spawn: map.pacman,
                frightened: 0,
            },
        );
        for &at in &map.ghosts {
            bots.spawn(
                ghost_team,
                Agent {
                    role: Role::Ghost,
                    pos: at,
                    spawn: at,
                    frightened: 0,
                },
            );
        }
        Ok(Self {
            bots,
            map,
            tick: 0,
            pacman,
            pacman_team,
            ghost_team,
            moved: false,
            ghosts_eaten: 0,
            outcome: None,
        })
    }

    fn bots(&self) -> &BotManager<Agent> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<Agent> {
        &mut self.bots
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::View(q) => Ok(q.reply(View {
                tick: self.tick,
                me: self.agent_view(bot),
                maze: self.map.rows(),
                pacman: self.agent_view(self.pacman),
                ghosts: self.ghosts().map(|id| self.agent_view(id)).collect(),
                pellets_left: self.map.pellets(),
            })),
        }
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        match action {
            Action::Move(a) => Ok(a.reply(self.move_agent(bot, a.dir)?)),
        }
    }

    fn end_step(&mut self, _bot: BotId) {
        self.moved = false;
    }

    fn bot_failed(&mut self, bot: BotId, failure: &BotFailure) {
        self.moved = false;
        if bot == self.pacman {
            self.end(Outcome::forfeit(
                self.ghost_team,
                format!("Pac-Man failed: {}", failure.detail()),
            ));
        }
    }

    fn end_tick(&mut self) {
        for id in self.ghosts().collect::<Vec<_>>() {
            let ghost = &mut self.bots[id];
            ghost.frightened = ghost.frightened.saturating_sub(1);
        }
        self.tick += 1;
    }

    fn status(&self) -> GameStatus {
        match &self.outcome {
            None => GameStatus::InProgress,
            Some(outcome) => GameStatus::Complete(outcome.clone()),
        }
    }

    fn tick_limit(&self, _max_ticks: u32) -> Outcome {
        Outcome::win(self.ghost_team).with_detail("Pac-Man ran out of time")
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            maze: self.map.rows(),
            agents: self
                .bots
                .ids()
                .into_iter()
                .map(|id| self.agent_view(id))
                .collect(),
            pacman_team: self.pacman_team.0,
            ghost_team: self.ghost_team.0,
            pellets_left: self.map.pellets(),
            ghosts_eaten: self.ghosts_eaten,
        }
    }
}
