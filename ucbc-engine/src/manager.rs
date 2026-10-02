//! A game's live bots: the one place ids are handed out and teams are recorded.

use std::ops::{Deref, DerefMut, Index};

use indexmap::IndexMap;

use crate::ids::{BotId, TeamId};

/// A bot's team and whatever the game keeps for it. Derefs to the game's data.
pub struct BotWrap<T> {
    team: TeamId,
    data: T,
}

impl<T> BotWrap<T> {
    pub fn team(&self) -> TeamId {
        self.team
    }
}

impl<T> Deref for BotWrap<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T> DerefMut for BotWrap<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

/// Live bots in spawn order. Ids count up from 0 and are never reused, so a removed
/// bot cannot come back. The engine steps bots in this order unless the game
/// overrides [`Game::schedule`](crate::Game::schedule).
pub struct BotManager<T> {
    next_id: u64,
    bots: IndexMap<BotId, BotWrap<T>>,
}

impl<T> BotManager<T> {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            bots: IndexMap::new(),
        }
    }

    /// Adds a bot; it steps from the next tick.
    pub fn schedule(&mut self, team: TeamId, data: T) -> BotId {
        let id = BotId(self.next_id);
        self.next_id += 1;
        self.bots.insert(id, BotWrap { team, data });
        id
    }

    pub fn remove(&mut self, id: BotId) -> Option<BotWrap<T>> {
        self.bots.shift_remove(&id)
    }

    pub fn get(&self, id: BotId) -> Option<&BotWrap<T>> {
        self.bots.get(&id)
    }

    /// Live ids in spawn order.
    pub fn ids(&self) -> Vec<BotId> {
        self.bots.keys().copied().collect()
    }
}

impl<T> Default for BotManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Panics on an id the manager does not have.
impl<T> Index<BotId> for BotManager<T> {
    type Output = BotWrap<T>;

    fn index(&self, id: BotId) -> &BotWrap<T> {
        &self.bots[&id]
    }
}
