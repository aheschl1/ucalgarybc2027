use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::bot::{Bot, BotFactory, BotResourceLimit, SpawnCtx, TeamSpec};
use crate::error::{BotFailure, EngineError};
use crate::ids::{BotId, BotRef, TeamId, TeamInfo};
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

/// The teams of a match and every live bot. Ids released this set stay dead so a
/// game cannot revive a bot by scheduling it again.
pub struct BotRegistry {
    teams: Vec<TeamEntry>,
    bots: HashMap<BotId, BotHandle>,
    dead: HashSet<BotId>,
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
            dead: HashSet::new(),
            set_seed: 0,
            limits,
        }
    }

    /// Releases every bot from the previous set and fixes the seed bots spawned in this
    /// set derive from.
    pub fn begin_set(&mut self, set_seed: u64) {
        self.bots.clear();
        self.dead.clear();
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

    pub fn is_dead(&self, id: BotId) -> bool {
        self.dead.contains(&id)
    }

    /// Validate a ref before scheduling or acquiring its runtime. A bot ID is owned
    /// by the team that first spawned it for the duration of this set.
    pub fn validate_ref(&self, bot: BotRef) -> Result<(), EngineError> {
        if !self.has_team(bot.team) {
            return Err(EngineError::Game(format!(
                "scheduled bot {} for unknown team {}",
                bot.id, bot.team
            )));
        }
        if self.is_dead(bot.id) {
            return Err(EngineError::Game(format!("scheduled dead bot {}", bot.id)));
        }
        if let Some(handle) = self.bots.get(&bot.id)
            && handle.team.id != bot.team
        {
            return Err(EngineError::Game(format!(
                "bot {} belongs to team {}, not team {}",
                bot.id, handle.team.id, bot.team
            )));
        }
        Ok(())
    }

    /// The bot for `bot`, created from its team's spec on first use.
    pub fn bot_mut(&mut self, bot: BotRef) -> Result<&mut BotHandle, BotLookupError> {
        self.validate_ref(bot).map_err(BotLookupError::Game)?;
        if !self.bots.contains_key(&bot.id) {
            let entry = &mut self.teams[bot.team.0 as usize];
            let ctx = SpawnCtx::new(
                bot,
                entry.info.clone(),
                bot_seed(self.set_seed, bot),
                self.limits,
            );
            let created = (entry.factory)(&ctx).map_err(BotLookupError::Bot)?;
            let handle = BotHandle {
                bot: created,
                team: ctx.team,
                seed: ctx.seed,
            };
            self.bots.insert(bot.id, handle);
        }
        Ok(self.bots.get_mut(&bot.id).expect("just inserted"))
    }

    pub fn despawn(&mut self, id: BotId) {
        self.bots.remove(&id);
        self.dead.insert(id);
    }
}
