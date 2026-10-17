use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use ucbc_engine::{Bot, BotFailure, StepCtx, StepResult};

use crate::vec_env::{AgentId, Event, Turn};

/// One env's line to the trainer. Only one of its bots steps at a time.
pub(crate) struct Link {
    pub env: usize,
    pub events: Sender<Event>,
    actions: Mutex<Receiver<Vec<Value>>>,
    observe: Value,
}

impl Link {
    pub fn new(
        env: usize,
        events: Sender<Event>,
        actions: Receiver<Vec<Value>>,
        observe: Value,
    ) -> Self {
        Self {
            env,
            events,
            actions: Mutex::new(actions),
            observe,
        }
    }

    /// `None` once the trainer is gone.
    fn wait(&self, turn: Turn) -> Option<Vec<Value>> {
        self.events.send(Event::Turn(turn)).ok()?;
        self.actions.lock().unwrap().recv().ok()
    }
}

pub(crate) struct ChannelBot {
    agent: AgentId,
    link: Arc<Link>,
}

impl ChannelBot {
    pub fn new(agent: AgentId, link: Arc<Link>) -> Self {
        Self { agent, link }
    }
}

impl Bot for ChannelBot {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let obs = match ctx.query(&self.link.observe) {
            Ok(obs) => obs,
            Err(e) => {
                let _ = self.link.events.send(Event::Failed {
                    env: self.link.env,
                    error: format!("observe: {e}"),
                });
                return StepResult::failed(BotFailure::exception("ObserveFailed", e.to_string()));
            }
        };
        let turn = Turn {
            agent: self.agent,
            team: ctx.team.id,
            tick: ctx.tick,
            obs,
        };
        let Some(actions) = self.link.wait(turn) else {
            return StepResult::failed(BotFailure::Crash("the trainer is gone".into()));
        };
        let replies = actions.iter().map(|a| ctx.act(a)).collect();
        let _ = self.link.events.send(Event::Acted {
            agent: self.agent,
            replies,
        });
        StepResult::ok()
    }
}

impl Drop for ChannelBot {
    fn drop(&mut self) {
        let _ = self.link.events.send(Event::Gone(self.agent));
    }
}
