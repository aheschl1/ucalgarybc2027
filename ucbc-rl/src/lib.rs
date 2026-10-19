//! Training environments over the engine. A [`VecEnv`] runs one set per env thread, back
//! to back. Each unit of a [`Seat::Policy`] team is a bot that observes through the same
//! query a sandboxed bot uses, then waits for the trainer's actions, so every env always
//! has exactly one unit waiting.

mod channel;
pub mod frontier;
mod ledger;
mod vec_env;
mod worker;

pub use ledger::Reward;
pub use vec_env::{AgentId, Batch, Event, Turn, VecEnv};
pub use worker::{EnvSpec, Recording, Seat};
