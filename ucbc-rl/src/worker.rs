use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use ucbc_engine::rng::set_seed;
use ucbc_engine::{
    BotResourceLimit, EngineError, GameRegistry, MatchConfig, MatchRunner, MatchSpec, SpawnCtx,
    TeamSpec,
};

use crate::channel::{ChannelBot, Link, Message};
use crate::vec_env::{AgentId, Event};

/// Who plays a team: the policy, or a team built fresh for each set.
#[derive(Clone)]
pub enum Seat {
    Policy,
    Team(Arc<dyn Fn() -> TeamSpec + Send + Sync>),
}

/// Write the replay of every `every`th set into `dir`.
#[derive(Clone)]
pub struct Recording {
    pub dir: PathBuf,
    pub every: u32,
}

/// One env: the set it plays, again and again with fresh seeds. Its game must keep a
/// [`score`](ucbc_engine::Game::score), which the rewards come from.
#[derive(Clone)]
pub struct EnvSpec {
    pub game: String,
    pub game_config: Option<Value>,
    pub max_ticks: u32,
    pub limits: BotResourceLimit,
    /// One per team, in team order; at least one is the policy.
    pub seats: Vec<Seat>,
    pub seed: u64,
    /// The query a policy unit observes with.
    pub observe: Value,
    pub record: Option<Recording>,
}

/// Plays sets until the trainer is gone; a failure or panic is the trainer's last event.
pub(crate) fn run(env: usize, spec: EnvSpec, registry: Arc<GameRegistry>, link: Arc<Link>) {
    let events = link.events.clone();
    let error = match panic::catch_unwind(AssertUnwindSafe(|| play(env, &spec, &registry, &link))) {
        Ok(None) => return,
        Ok(Some(error)) => error,
        Err(panic) => format!("panicked: {}", message(&*panic)),
    };
    let _ = events.send(Message::Failed { env, error });
}

fn message(panic: &(dyn Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown")
}

/// `None` once the trainer is gone, else why the env stopped.
fn play(env: usize, spec: &EnvSpec, registry: &GameRegistry, link: &Arc<Link>) -> Option<String> {
    let mut episode = 0;
    loop {
        let teams = spec
            .seats
            .iter()
            .map(|seat| match seat {
                Seat::Policy => {
                    let link = link.clone();
                    TeamSpec::rust("policy", move |ctx: &SpawnCtx| {
                        let agent = AgentId {
                            env,
                            episode,
                            bot: ctx.bot,
                        };
                        ChannelBot::new(agent, link.clone())
                    })
                }
                Seat::Team(make) => make(),
            })
            .collect();
        let seed = set_seed(set_seed(spec.seed, env as u32), episode);
        let mut config =
            MatchConfig::new(&spec.game, 1, seed, 0, spec.limits).max_ticks(spec.max_ticks);
        if let Some(game_config) = &spec.game_config {
            config = config.game_config(game_config.clone());
        }
        let (ticks, ends) = (link.events.clone(), link.events.clone());
        let mut match_spec = MatchSpec::new(format!("env{env}-{episode}"), config, teams)
            .record(false)
            .on_tick(Box::new(move |_, moment, scores| {
                let scores = scores.to_vec();
                let _ = ticks.send(Message::Score {
                    env,
                    moment,
                    scores,
                });
            }))
            // Before the runner drops the units still in the set.
            .on_set(Box::new(move |set| {
                let result = set.result.clone();
                ends.send(Message::Event(Event::SetEnd {
                    env,
                    episode,
                    result,
                }))
                .map_err(|e| EngineError::Callback(Box::new(e)))
            }));
        if let Some(r) = &spec.record
            && episode.is_multiple_of(r.every)
        {
            let path = r.dir.join(format!("env{env}-{episode}.json.gz"));
            match_spec = match_spec.record(true).replay_path(path);
        }
        match MatchRunner::new(registry, match_spec).and_then(MatchRunner::run) {
            Ok(_) => episode += 1,
            Err(EngineError::Callback(_)) => return None,
            Err(e) => return Some(e.to_string()),
        }
    }
}
