use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use serde_json::Value;
use ucbc_engine::{ActionError, BotId, EngineError, GameRegistry, SetResult, TeamId};

use crate::channel::{Link, Message};
use crate::ledger::{Ledger, Reward};
use crate::worker::{self, EnvSpec, Seat};

/// One unit's whole life: its bot in one set of one env.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AgentId {
    pub env: usize,
    pub episode: u32,
    pub bot: BotId,
}

/// A unit waiting for its actions.
#[derive(Debug)]
pub struct Turn {
    pub agent: AgentId,
    pub team: TeamId,
    pub tick: u32,
    pub obs: Value,
    /// For its previous turn; nothing on its first
    pub reward: Reward,
}

#[derive(Debug)]
pub enum Event {
    /// The game's reply to each action of the agent's last turn.
    Acted {
        agent: AgentId,
        replies: Vec<Result<Value, ActionError>>,
    },
    /// The game removed the agent, or its set ended; the reward is for its last turn.
    Gone { agent: AgentId, reward: Reward },
    /// Before the `Gone` of the units still in the set.
    SetEnd {
        env: usize,
        episode: u32,
        result: SetResult,
    },
}

/// What the trainer gets back from each call.
pub struct Batch {
    /// One per env, in env order.
    pub turns: Vec<Turn>,
    /// Everything else since the last batch, in order.
    pub events: Vec<Event>,
}

/// Many envs, each with exactly one unit waiting. After an error it can only be dropped.
pub struct VecEnv {
    actions: Vec<Sender<Vec<Value>>>,
    events: Receiver<Message>,
    workers: Vec<JoinHandle<()>>,
    ledgers: Vec<Ledger>,
    started: bool,
}

impl VecEnv {
    pub fn new(registry: Arc<GameRegistry>, specs: Vec<EnvSpec>) -> Result<Self, EngineError> {
        if let Some(env) = specs
            .iter()
            .position(|s| !s.seats.iter().any(|seat| matches!(seat, Seat::Policy)))
        {
            return Err(EngineError::Config(format!("env {env} has no policy seat")));
        }
        let (events_tx, events) = mpsc::channel();
        let mut actions = Vec::with_capacity(specs.len());
        let mut workers = Vec::with_capacity(specs.len());
        for (env, spec) in specs.into_iter().enumerate() {
            let (tx, rx) = mpsc::channel();
            let link = Arc::new(Link::new(env, events_tx.clone(), rx, spec.observe.clone()));
            let registry = registry.clone();
            let worker = thread::Builder::new()
                .name(format!("env-{env}"))
                .spawn(move || worker::run(env, spec, registry, link))?;
            actions.push(tx);
            workers.push(worker);
        }
        let ledgers = (0..actions.len()).map(|_| Default::default()).collect();
        Ok(Self {
            actions,
            events,
            workers,
            ledgers,
            started: false,
        })
    }

    pub fn num_envs(&self) -> usize {
        self.actions.len()
    }

    /// The first unit waiting in each env. Call once, before the first `step`.
    pub fn reset(&mut self) -> Result<Batch, EngineError> {
        if self.started {
            return Err(EngineError::Config("reset twice".into()));
        }
        self.started = true;
        self.collect()
    }

    /// Hands each env's waiting unit its actions, in env order, and waits for the next.
    pub fn step(&mut self, actions: Vec<Vec<Value>>) -> Result<Batch, EngineError> {
        if !self.started {
            return Err(EngineError::Config("step before reset".into()));
        }
        if actions.len() != self.num_envs() {
            return Err(EngineError::Config(format!(
                "{} actions for {} envs",
                actions.len(),
                self.num_envs()
            )));
        }
        for (env, (tx, a)) in self.actions.iter().zip(actions).enumerate() {
            tx.send(a)
                .map_err(|_| EngineError::Game(format!("env {env} stopped")))?;
        }
        self.collect()
    }

    fn collect(&mut self) -> Result<Batch, EngineError> {
        let mut turns: Vec<Option<Turn>> = (0..self.num_envs()).map(|_| None).collect();
        let mut waiting = turns.len();
        let mut events = Vec::new();
        while waiting > 0 {
            let message = self
                .events
                .recv()
                .map_err(|_| EngineError::Game("every env stopped".into()))?;
            match message {
                Message::Score {
                    env,
                    moment,
                    scores,
                } => self.ledgers[env].tick(moment, &scores),
                Message::Turn {
                    agent,
                    team,
                    tick,
                    obs,
                } => {
                    let reward = self.ledgers[agent.env].turn(agent.bot, team);
                    turns[agent.env] = Some(Turn {
                        agent,
                        team,
                        tick,
                        obs,
                        reward,
                    });
                    waiting -= 1;
                }
                Message::Gone(agent) => {
                    let reward = self.ledgers[agent.env].gone(agent.bot);
                    events.push(Event::Gone { agent, reward });
                }
                Message::Event(event) => {
                    if let Event::SetEnd { env, result, .. } = &event {
                        self.ledgers[*env].end(result.winner_team);
                    }
                    events.push(event);
                }
                Message::Failed { env, error } => {
                    return Err(EngineError::Game(format!("env {env}: {error}")));
                }
            }
        }
        Ok(Batch {
            turns: turns.into_iter().map(Option::unwrap).collect(),
            events,
        })
    }
}

/// Waiting units find the trainer gone and fail; each worker stops when it can't report
/// its set's end, so this waits for the sets in play to end.
impl Drop for VecEnv {
    fn drop(&mut self) {
        self.actions.clear();
        drop(std::mem::replace(&mut self.events, mpsc::channel().1));
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
