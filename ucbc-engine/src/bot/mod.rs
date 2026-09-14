//! A team is one code submission. The engine creates one [`Bot`] per bot id from the
//! owning team's [`TeamSpec`] when the game first schedules it, and releases it when
//! the game despawns it or the set ends.

pub mod registry;

use std::sync::Arc;

use crate::error::BotFailure;
use crate::ids::{BotRef, TeamInfo};
use crate::step::{StepCtx, StepResult};

pub trait Bot: Send {
    /// Run one step. Returning ends the step.
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult;

    /// Called when the bot is despawned.
    fn shutdown(&mut self) {}
}

/// What the engine knows about a bot when it creates it.
pub struct SpawnCtx {
    pub bot: BotRef,
    pub team: Arc<TeamInfo>,
    pub seed: u64,
}

impl SpawnCtx {
    pub fn new(bot: BotRef, team: Arc<TeamInfo>, seed: u64) -> Self {
        Self { bot, team, seed }
    }
}

pub type RustBotFactory = Box<dyn Fn(&SpawnCtx) -> Box<dyn Bot> + Send + Sync>;

pub enum TeamKind {
    Rust(RustBotFactory),
    /// Every bot this team owns fails with this failure.
    Unavailable(BotFailure),
}

pub struct TeamSpec {
    pub name: String,
    pub kind: TeamKind,
}

impl TeamSpec {
    pub fn rust<F, B>(name: impl Into<String>, make: F) -> Self
    where
        F: Fn(&SpawnCtx) -> B + Send + Sync + 'static,
        B: Bot + 'static,
    {
        Self {
            name: name.into(),
            kind: TeamKind::Rust(Box::new(move |ctx| Box::new(make(ctx)) as Box<dyn Bot>)),
        }
    }

    pub fn unavailable(name: impl Into<String>, failure: BotFailure) -> Self {
        Self {
            name: name.into(),
            kind: TeamKind::Unavailable(failure),
        }
    }
}
