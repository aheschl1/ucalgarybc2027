use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use ucbc_engine::{Bot, BotFailure, StepCtx, StepResult, TeamId, TickMoment};

use crate::vec_env::{AgentId, Event};

/// What an env sends the trainer.
pub(crate) enum Message {
    Turn {
        agent: AgentId,
        team: TeamId,
        tick: u32,
        obs: Value,
    },
    /// Passed on as it is.
    Event(Event),
    Gone(AgentId),
    Score {
        env: usize,
        moment: TickMoment,
        scores: Vec<i64>,
    },
    Failed {
        env: usize,
        error: String,
    },
}

/// One env's line to the trainer. Only one of its bots steps at a time.
pub(crate) struct Link {
    pub env: usize,
    pub events: Sender<Message>,
    actions: Mutex<Receiver<Vec<Value>>>,
    observe: Value,
}

impl Link {
    pub fn new(
        env: usize,
        events: Sender<Message>,
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
    fn wait(&self, turn: Message) -> Option<Vec<Value>> {
        self.events.send(turn).ok()?;
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
                let _ = self.link.events.send(Message::Failed {
                    env: self.link.env,
                    error: format!("observe: {e}"),
                });
                return StepResult::failed(BotFailure::exception("ObserveFailed", e.to_string()));
            }
        };
        let turn = Message::Turn {
            agent: self.agent,
            team: ctx.team.id,
            tick: ctx.tick,
            obs,
        };
        let Some(actions) = self.link.wait(turn) else {
            return StepResult::failed(BotFailure::Crash("the trainer is gone".into()));
        };
        let replies = actions.iter().map(|a| ctx.act(a)).collect();
        let acted = Event::Acted {
            agent: self.agent,
            replies,
        };
        let _ = self.link.events.send(Message::Event(acted));
        StepResult::ok()
    }
}

impl Drop for ChannelBot {
    fn drop(&mut self) {
        let _ = self.link.events.send(Message::Gone(self.agent));
    }
}
