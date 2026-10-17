use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use serde_json::Value;
use ucbc_engine::{ActionError, BotId, EngineError, GameRegistry, SetResult, TeamId};

use crate::channel::Link;
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
}

#[derive(Debug)]
pub enum Event {
    Turn(Turn),
    /// The game's reply to each action of the agent's last turn.
    Acted {
        agent: AgentId,
        replies: Vec<Result<Value, ActionError>>,
    },
    /// The game removed the agent, or its set ended.
    Gone(AgentId),
    /// After the `Gone` of every unit of the set.
    SetEnd {
        env: usize,
        episode: u32,
        result: SetResult,
    },
    /// The env stopped; [`VecEnv`] returns it as an error.
    Failed {
        env: usize,
        error: String,
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
    events: Receiver<Event>,
    workers: Vec<JoinHandle<()>>,
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
        Ok(Self {
            actions,
            events,
            workers,
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
            let event = self
                .events
                .recv()
                .map_err(|_| EngineError::Game("every env stopped".into()))?;
            match event {
                Event::Turn(turn) => {
                    let env = turn.agent.env;
                    turns[env] = Some(turn);
                    waiting -= 1;
                }
                Event::Failed { env, error } => {
                    return Err(EngineError::Game(format!("env {env}: {error}")));
                }
                event => events.push(event),
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
