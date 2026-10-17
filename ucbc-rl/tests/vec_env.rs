use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Value, json};
use ucbc_engine::{BotResourceLimit, EngineError, GameRegistry, StepResult, TeamId, TeamSpec};
use ucbc_frontier::Frontier;
use ucbc_rl::{AgentId, Batch, EnvSpec, Event, Recording, Seat, VecEnv};

fn registry() -> Arc<GameRegistry> {
    let mut registry = GameRegistry::new();
    registry.register::<Frontier>();
    Arc::new(registry)
}

fn env(seats: Vec<Seat>) -> EnvSpec {
    EnvSpec {
        game: "frontier".into(),
        game_config: Some(json!({"size": 64, "flat": true})),
        max_ticks: 10,
        limits: BotResourceLimit::new(500, 1 << 30),
        seats,
        seed: 7,
        observe: json!({"type": "observe"}),
        record: None,
    }
}

fn idle() -> Seat {
    Seat::Team(Arc::new(|| TeamSpec::rust("idle", |_: &_| Idle)))
}

struct Idle;

impl ucbc_engine::Bot for Idle {
    fn step(&mut self, _: &mut ucbc_engine::StepCtx<'_>) -> StepResult {
        StepResult::ok()
    }
}

/// The commander spawns once, then everyone walks in a circle.
fn act(turn: usize, obs: &Value) -> Vec<Value> {
    let dir = ["up", "right", "down", "left"][turn % 4];
    let mut actions = vec![json!({"type": "move", "dir": dir})];
    if obs["me"]["kind"] == "commander" && turn == 0 {
        actions.push(json!({"type": "spawn"}));
    }
    actions
}

/// Steps until every env has finished `sets` sets, checking each batch as it comes.
fn play(venv: &mut VecEnv, sets: u32) -> Vec<Event> {
    let mut all = Vec::new();
    let mut live: HashSet<AgentId> = HashSet::new();
    let mut turns: HashMap<AgentId, usize> = HashMap::new();
    let mut batch = venv.reset().unwrap();
    loop {
        let Batch {
            turns: waiting,
            events,
        } = batch;
        for event in &events {
            match event {
                Event::Gone(agent) => assert!(live.remove(agent), "{agent:?} gone twice"),
                Event::SetEnd { env, episode, .. } => assert!(
                    !live.iter().any(|a| a.env == *env && a.episode == *episode),
                    "env {env} ended set {episode} with units left"
                ),
                Event::Acted { replies, .. } => {
                    assert!(!replies.is_empty());
                    // A move may be blocked; a spawn, the second action, may not.
                    assert!(replies.get(1).is_none_or(Result::is_ok), "{replies:?}");
                }
                _ => {}
            }
        }
        all.extend(events);
        let done = (0..venv.num_envs()).all(|env| {
            all.iter()
                .filter(|e| matches!(e, Event::SetEnd { env: e, .. } if *e == env))
                .count()
                >= sets as usize
        });
        if done {
            return all;
        }
        assert_eq!(waiting.len(), venv.num_envs());
        let mut actions = Vec::new();
        for (env, turn) in waiting.iter().enumerate() {
            assert_eq!(turn.agent.env, env);
            live.insert(turn.agent);
            let n = turns.entry(turn.agent).or_default();
            actions.push(act(*n, &turn.obs));
            *n += 1;
        }
        batch = venv.step(actions).unwrap();
    }
}

#[test]
fn self_play_hands_out_one_unit_per_env_and_restarts_each_set() {
    let specs = (0..4)
        .map(|_| env(vec![Seat::Policy, Seat::Policy]))
        .collect();
    let mut venv = VecEnv::new(registry(), specs).unwrap();
    let events = play(&mut venv, 3);

    for env in 0..4 {
        let ends: Vec<u32> = events
            .iter()
            .filter_map(|e| match e {
                Event::SetEnd {
                    env: e,
                    episode,
                    result,
                } if *e == env => {
                    assert_eq!(result.ticks, 10);
                    Some(*episode)
                }
                _ => None,
            })
            .collect();
        assert_eq!(ends[..3], [0, 1, 2]);
    }
    // Both teams' units waited, and each commander's spawn made a worker that waited too.
    let acted: HashSet<AgentId> = events
        .iter()
        .filter_map(|e| match e {
            Event::Acted { agent, .. } if agent.env == 0 && agent.episode == 0 => Some(*agent),
            _ => None,
        })
        .collect();
    assert_eq!(acted.len(), 4);
}

#[test]
fn only_policy_units_wait() {
    let mut venv = VecEnv::new(registry(), vec![env(vec![idle(), Seat::Policy])]).unwrap();
    let mut batch = venv.reset().unwrap();
    for _ in 0..30 {
        assert_eq!(batch.turns[0].team, TeamId(1));
        let actions = vec![act(1, &batch.turns[0].obs)];
        batch = venv.step(actions).unwrap();
    }
}

#[test]
fn every_nth_set_is_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let mut spec = env(vec![Seat::Policy, idle()]);
    spec.record = Some(Recording {
        dir: dir.path().into(),
        every: 2,
    });
    let mut venv = VecEnv::new(registry(), vec![spec]).unwrap();
    play(&mut venv, 3);
    let replay = ucbc_engine::replay::read_replay(&dir.path().join("env0-0.json.gz")).unwrap();
    assert_eq!(replay.sets[0].ticks.len(), 10);
    assert!(dir.path().join("env0-2.json.gz").exists());
    assert!(!dir.path().join("env0-1.json.gz").exists());
}

#[test]
fn bad_specs_and_action_counts_are_errors() {
    let Err(EngineError::Config(_)) = VecEnv::new(registry(), vec![env(vec![idle(), idle()])])
    else {
        panic!("an env without a policy seat");
    };
    let mut venv = VecEnv::new(registry(), vec![env(vec![Seat::Policy, idle()])]).unwrap();
    assert!(matches!(
        venv.step(vec![vec![]]),
        Err(EngineError::Config(_))
    ));
    venv.reset().unwrap();
    assert!(matches!(venv.reset(), Err(EngineError::Config(_))));
    assert!(matches!(venv.step(vec![]), Err(EngineError::Config(_))));

    let mut spec = env(vec![Seat::Policy, idle()]);
    spec.game = "nope".into();
    let mut venv = VecEnv::new(registry(), vec![spec]).unwrap();
    let Err(EngineError::Game(error)) = venv.reset() else {
        panic!("an unknown game");
    };
    assert!(error.contains("nope"), "{error}");
}

fn reset_error(spec: EnvSpec) -> String {
    let mut venv = VecEnv::new(registry(), vec![spec]).unwrap();
    match venv.reset() {
        Err(EngineError::Game(error)) => error,
        other => panic!("expected an error, got {:?}", other.map(|b| b.turns)),
    }
}

#[test]
fn a_failing_observe_or_a_panicking_team_is_an_error() {
    let mut spec = env(vec![Seat::Policy, idle()]);
    spec.observe = json!({"type": "nope"});
    let error = reset_error(spec);
    assert!(error.starts_with("env 0: observe:"), "{error}");

    let boom = Seat::Team(Arc::new(|| panic!("boom")));
    let error = reset_error(env(vec![Seat::Policy, boom]));
    assert_eq!(error, "env 0: panicked: boom");
}

#[test]
fn an_env_failing_mid_run_ends_the_vec_env() {
    let mut broken = env(vec![Seat::Policy, idle()]);
    broken.record = Some(Recording {
        dir: "/nonexistent/ucbc-rl".into(),
        every: 1,
    });
    let specs = vec![env(vec![Seat::Policy, idle()]), broken];
    let mut venv = VecEnv::new(registry(), specs).unwrap();
    let mut batch = venv.reset().unwrap();
    let error = loop {
        let actions = batch.turns.iter().map(|t| act(1, &t.obs)).collect();
        match venv.step(actions) {
            Ok(next) => batch = next,
            Err(EngineError::Game(error)) => break error,
            Err(e) => panic!("{e}"),
        }
    };
    assert!(error.starts_with("env 1: "), "{error}");
}

#[test]
fn envs_and_sets_are_seeded_apart_and_alike_across_runs() {
    let generated = || {
        let mut spec = env(vec![Seat::Policy, idle()]);
        spec.game_config = Some(json!({"size": 64}));
        spec
    };
    let first_tiles = || {
        let mut venv = VecEnv::new(registry(), vec![generated(), generated()]).unwrap();
        let batch = venv.reset().unwrap();
        let tiles: Vec<Value> = batch.turns.iter().map(|t| t.obs["tiles"].clone()).collect();
        tiles
    };
    let (a, b) = (first_tiles(), first_tiles());
    assert_eq!(a, b);
    assert_ne!(a[0], a[1]);
}
