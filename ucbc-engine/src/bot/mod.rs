//! A team is one code submission. The engine creates one [`Bot`] per bot id from the
//! owning team's [`TeamSpec`] when the game first schedules it, and releases it when
//! the game despawns it or the set ends.

pub mod registry;

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::BotFailure;
use crate::ids::{BotRef, TeamInfo};
use crate::step::{StepCtx, StepResult};

pub trait Bot: Send {
    /// Run one step. Returning ends the step.
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult;

    /// Called when the bot is despawned.
    fn shutdown(&mut self) {}
}

/// What one bot may use, set per match. The engine hands it to the bot's runtime at
/// spawn; enforcement is the runtime's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotResourceLimit {
    /// Wall-clock budget, in milliseconds, for loading the team's code and for each step.
    pub step_ms: u64,
    /// Bytes the bot's runtime may hold.
    pub memory_bytes: u64,
}

impl BotResourceLimit {
    pub const fn new(step_ms: u64, memory_bytes: u64) -> Self {
        Self {
            step_ms,
            memory_bytes,
        }
    }

    pub fn step_time(&self) -> Duration {
        Duration::from_millis(self.step_ms)
    }
}

/// What the engine knows about a bot when it creates it.
pub struct SpawnCtx {
    pub bot: BotRef,
    pub team: Arc<TeamInfo>,
    pub seed: u64,
    pub limits: BotResourceLimit,
}

impl SpawnCtx {
    pub fn new(bot: BotRef, team: Arc<TeamInfo>, seed: u64, limits: BotResourceLimit) -> Self {
        Self {
            bot,
            team,
            seed,
            limits,
        }
    }
}

/// Creates a team's bots. An `Err` fails that bot at its first step.
pub type BotFactory = Box<dyn FnMut(&SpawnCtx) -> Result<Box<dyn Bot>, BotFailure> + Send>;

pub struct TeamSpec {
    pub name: String,
    pub factory: BotFactory,
}

impl TeamSpec {
    pub fn new(name: impl Into<String>, factory: BotFactory) -> Self {
        Self {
            name: name.into(),
            factory,
        }
    }

    /// A team whose bots are plain Rust values.
    pub fn rust<F, B>(name: impl Into<String>, mut make: F) -> Self
    where
        F: FnMut(&SpawnCtx) -> B + Send + 'static,
        B: Bot + 'static,
    {
        Self::new(
            name,
            Box::new(move |ctx| Ok(Box::new(make(ctx)) as Box<dyn Bot>)),
        )
    }

    /// A team whose code cannot run; every bot it owns fails with `failure`.
    pub fn unavailable(name: impl Into<String>, failure: BotFailure) -> Self {
        Self::new(name, Box::new(move |_| Err(failure.clone())))
    }
}
