use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::bot::{Bot, BotFactory, SpawnCtx, TeamSpec};
use crate::error::BotFailure;
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

/// The teams of a match and every live bot. Ids released this set stay dead so a
/// game cannot revive a bot by scheduling it again.
pub struct BotRegistry {
    teams: Vec<TeamEntry>,
    bots: HashMap<BotId, BotHandle>,
    dead: HashSet<BotId>,
    set_seed: u64,
}

impl BotRegistry {
    pub fn new(specs: Vec<TeamSpec>) -> Self {
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
        }
    }

    /// Releases every bot from the previous set and fixes the seed bots spawned in this
    /// set derive from.
    pub fn begin_set(&mut self, set_seed: u64) {
        self.despawn_all();
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

    /// The bot for `bot`, created from its team's spec on first use. The team must
    /// exist and the bot must not be dead.
    pub fn bot_mut(&mut self, bot: BotRef) -> Result<&mut BotHandle, BotFailure> {
        if !self.bots.contains_key(&bot.id) {
            let entry = &mut self.teams[bot.team.0 as usize];
            let ctx = SpawnCtx::new(bot, entry.info.clone(), bot_seed(self.set_seed, bot));
            let created = (entry.factory)(&ctx)?;
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
        if let Some(mut handle) = self.bots.remove(&id) {
            handle.bot.shutdown();
        }
        self.dead.insert(id);
    }

    fn despawn_all(&mut self) {
        for (_, mut handle) in self.bots.drain() {
            handle.bot.shutdown();
        }
    }
}

impl Drop for BotRegistry {
    fn drop(&mut self) {
        self.despawn_all();
    }
}
