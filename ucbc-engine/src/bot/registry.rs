use std::collections::HashMap;
use std::sync::Arc;

use crate::bot::{Bot, BotFactory, BotResourceLimit, SpawnCtx, TeamSpec};
use crate::error::{BotFailure, EngineError};
use crate::ids::{BotId, TeamId, TeamInfo};
use crate::rng::bot_seed;

struct TeamEntry {
    info: Arc<TeamInfo>,
    factory: BotFactory,
}

/// A live bot and what the engine derived for it at spawn.
pub struct BotHandle {
    pub bot: Box<dyn Bot>,
    pub team: Arc<TeamInfo>,
    pub seed: u64,
}

pub enum BotLookupError {
    Game(EngineError),
    Bot(BotFailure),
}

/// The teams of a match and the runtime of every live bot. Which bots exist, and
/// their teams, is the game's [`BotManager`](crate::BotManager); this follows it.
pub struct BotRegistry {
    teams: Vec<TeamEntry>,
    bots: HashMap<BotId, BotHandle>,
    set_seed: u64,
    limits: BotResourceLimit,
}

impl BotRegistry {
    pub fn new(specs: Vec<TeamSpec>, limits: BotResourceLimit) -> Self {
        let teams = specs
            .into_iter()
            .enumerate()
            .map(|(i, spec)| TeamEntry {
                info: Arc::new(TeamInfo::new(TeamId(i as u32), spec.name)),
                factory: spec.factory,
            })
            .collect();
        Self {
            teams,
            bots: HashMap::new(),
            set_seed: 0,
            limits,
        }
    }

    /// Releases every bot from the previous set and fixes the seed bots spawned in this
    /// set derive from.
    pub fn begin_set(&mut self, set_seed: u64) {
        self.bots.clear();
        self.set_seed = set_seed;
    }

    pub fn teams(&self) -> Vec<TeamInfo> {
        self.teams.iter().map(|t| (*t.info).clone()).collect()
    }

    pub fn team_info(&self, id: TeamId) -> Arc<TeamInfo> {
        self.teams[id.0 as usize].info.clone()
    }

    pub fn has_team(&self, id: TeamId) -> bool {
        (id.0 as usize) < self.teams.len()
    }

    /// The runtime for `bot`, created from its team's spec on first use.
    pub fn bot_mut(&mut self, bot: BotId, team: TeamId) -> Result<&mut BotHandle, BotLookupError> {
        if !self.has_team(team) {
            return Err(BotLookupError::Game(EngineError::Game(format!(
                "bot {bot} belongs to unknown team {team}"
            ))));
        }
        if !self.bots.contains_key(&bot) {
            let entry = &mut self.teams[team.0 as usize];
            let ctx = SpawnCtx::new(
                bot,
                entry.info.clone(),
                bot_seed(self.set_seed, team, bot),
                self.limits,
            );
            let created = (entry.factory)(&ctx).map_err(BotLookupError::Bot)?;
            let handle = BotHandle {
                bot: created,
                team: ctx.team,
                seed: ctx.seed,
            };
            self.bots.insert(bot, handle);
        }
        Ok(self.bots.get_mut(&bot).expect("just inserted"))
    }

    /// Drops the runtime of every bot `live` rejects.
    pub fn retain(&mut self, live: impl Fn(BotId) -> bool) {
        self.bots.retain(|&id, _| live(id));
    }
}
