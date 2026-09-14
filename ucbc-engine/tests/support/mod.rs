//! Test doubles: a trivial game and closure-driven bots.

pub mod counting_game;

use std::sync::Arc;

use ucbc_engine::{Bot, StepCtx, StepResult, TeamSpec};

type StepFn = Arc<dyn Fn(&mut StepCtx<'_>) -> StepResult + Send + Sync>;

/// A bot whose step is a shared closure. One closure serves every bot of the team.
pub struct ScriptedBot {
    step: StepFn,
}

impl ScriptedBot {
    pub fn new(step: StepFn) -> Self {
        Self { step }
    }
}

impl Bot for ScriptedBot {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        (self.step)(ctx)
    }
}

pub fn scripted<F>(name: &str, step: F) -> TeamSpec
where
    F: Fn(&mut StepCtx<'_>) -> StepResult + Send + Sync + 'static,
{
    let step: StepFn = Arc::new(step);
    TeamSpec::rust(name, move |_ctx| ScriptedBot::new(step.clone()))
}

/// A bot built from a per-bot closure (for factories that want per-bot state).
pub struct ScriptedBotWith {
    step: Box<dyn FnMut(&mut StepCtx<'_>) -> StepResult + Send>,
}

impl ScriptedBotWith {
    pub fn new<F>(step: F) -> Self
    where
        F: FnMut(&mut StepCtx<'_>) -> StepResult + Send + 'static,
    {
        Self {
            step: Box::new(step),
        }
    }
}

impl Bot for ScriptedBotWith {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        (self.step)(ctx)
    }
}
