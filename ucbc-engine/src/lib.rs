//! Game-agnostic match engine. A game plugs in through [`Game`] and [`GameFactory`];
//! teams plug in through [`Bot`]. [`MatchRunner`] drives sets and ticks, validates every
//! action through the game, and writes a JSON replay.

pub mod bot;
pub mod error;
pub mod game;
pub mod ids;
pub mod payload;
pub mod query;
pub mod replay;
pub mod rng;
pub mod runner;
pub mod step;
pub mod summary;

pub use bot::{Bot, BotFactory, BotResourceLimit, SpawnCtx, TeamSpec, registry::BotRegistry};
pub use error::{ActionError, BotFailure, DecodeError, EngineError, QueryError};
pub use game::{DynGame, Game, GameApi, GameFactory, GameRegistry, GameStatus, Outcome, SetSetup};
pub use ids::{BotId, BotRef, TeamId, TeamInfo};
pub use query::{Answer, Query};
pub use replay::{
    MatchConfig, MatchResult, Reason, Replay, SetReplay, SetResult, Step, Tick, Usage,
};
pub use runner::{MatchReport, MatchRunner, MatchSpec, SetHook};
pub use step::{StepCtx, StepResult};
pub use summary::Summary;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
