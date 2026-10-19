use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Value, json};
use ucbc_engine::{
    BotId, BotResourceLimit, EngineError, GameRegistry, StepResult, TeamId, TeamSpec,
};
use ucbc_frontier::{Frontier, Kind, Observation};
use ucbc_rl::{AgentId, Batch, EnvSpec, Event, Recording, Reward, Seat, Turn, VecEnv};

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

/// Steps until every env has finished `sets` sets, checking each batch as it comes. Returns
/// every turn and every event, each in order.
fn play(venv: &mut VecEnv, sets: u32) -> (Vec<Turn>, Vec<Event>) {
    let (mut handed, mut all) = (Vec::new(), Vec::new());
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
                Event::Gone { agent, .. } => assert!(live.remove(agent), "{agent:?} gone twice"),
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
            return (handed, all);
        }
        assert_eq!(waiting.len(), venv.num_envs());
        let mut actions = Vec::new();
        for (env, turn) in waiting.into_iter().enumerate() {
            assert_eq!(turn.agent.env, env);
            // Every unit of a set is gone before the next set's first turn.
            assert!(
                !live
                    .iter()
                    .any(|a| a.env == env && a.episode < turn.agent.episode),
                "env {env} started set {} with units left",
                turn.agent.episode
            );
            live.insert(turn.agent);
            let n = turns.entry(turn.agent).or_default();
            actions.push(act(*n, &turn.obs));
            *n += 1;
            handed.push(turn);
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
    let (_, events) = play(&mut venv, 3);

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
    // Each set's replay is written just after its end is reported.
    drop(venv);
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
    // Its opponent can only be made once: the second set fails.
    let made = std::sync::atomic::AtomicBool::new(false);
    let once = Seat::Team(Arc::new(move || {
        assert!(
            !made.swap(true, std::sync::atomic::Ordering::Relaxed),
            "made twice"
        );
        TeamSpec::rust("idle", |_: &_| Idle)
    }));
    let specs = vec![
        env(vec![Seat::Policy, idle()]),
        env(vec![Seat::Policy, once]),
    ];
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
    assert_eq!(error, "env 1: panicked: made twice");
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

#[test]
fn rewards_charge_a_spawn_to_its_commander_and_end_with_the_result() {
    let mut venv = VecEnv::new(registry(), vec![env(vec![Seat::Policy, Seat::Policy])]).unwrap();
    let (turns, events) = play(&mut venv, 1);
    let mut teams: HashMap<AgentId, TeamId> = HashMap::new();
    let mut rewards: HashMap<AgentId, Vec<Reward>> = HashMap::new();
    for t in turns.iter().filter(|t| t.agent.episode == 0) {
        teams.insert(t.agent, t.team);
        rewards.entry(t.agent).or_default().push(t.reward);
    }
    let mut winner = None;
    for event in &events {
        match event {
            Event::Gone { agent, reward } if agent.episode == 0 => {
                rewards.entry(*agent).or_default().push(*reward);
            }
            Event::SetEnd {
                episode: 0, result, ..
            } => winner = result.winner_team,
            _ => {}
        }
    }
    let commander = |team| AgentId {
        env: 0,
        episode: 0,
        bot: BotId(team),
    };
    for team in [0, 1] {
        let r = &rewards[&commander(team)];
        assert_eq!(r[0], Reward::default());
        assert_eq!(r[1].own, -5.0);
    }
    let winner = winner.expect("a winner at the tick limit");
    for (agent, r) in &rewards {
        let expected = if teams[agent] == winner { 1.0 } else { -1.0 };
        assert_eq!(r.last().unwrap().result, expected, "{agent:?}");
        assert!(r[..r.len() - 1].iter().all(|r| r.result == 0.0));
    }
}

/// Steps an env whose one policy unit does nothing until a batch has an event `f` picks;
/// returns that batch's events.
fn until(venv: &mut VecEnv, f: impl Fn(&Event) -> bool) -> Vec<Event> {
    let mut batch = venv.reset().unwrap();
    while !batch.events.iter().any(&f) {
        batch = venv.step(vec![vec![]]).unwrap();
    }
    batch.events
}

/// Team 0's worker walks under team 1's commander and hits it until it dies.
struct Hunter;

impl ucbc_engine::Bot for Hunter {
    fn step(&mut self, ctx: &mut ucbc_engine::StepCtx<'_>) -> StepResult {
        let seen: Observation =
            serde_json::from_value(ctx.query(&json!({"type": "observe"})).unwrap()).unwrap();
        let act = match (seen.me.kind, seen.me.pos.y, seen.me.pos.x) {
            (Kind::Commander, ..) if ctx.tick == 0 => json!({"type": "spawn"}),
            (Kind::Commander, ..) => return StepResult::ok(),
            (_, y, _) if y < 34 => json!({"type": "move", "dir": "down"}),
            (_, _, x) if x < 47 => json!({"type": "move", "dir": "right"}),
            _ => json!({"type": "attack", "x": 47, "y": 33}),
        };
        ctx.act(&act).unwrap();
        StepResult::ok()
    }
}

#[test]
fn a_unit_killed_in_another_s_step_is_charged_its_death_and_gets_no_result() {
    let hunter = Seat::Team(Arc::new(|| TeamSpec::rust("hunter", |_: &_| Hunter)));
    let mut spec = env(vec![hunter, Seat::Policy]);
    spec.max_ticks = 200;
    let mut venv = VecEnv::new(registry(), vec![spec]).unwrap();
    let events = until(&mut venv, |e| matches!(e, Event::Gone { .. }));
    let gone = events.iter().find_map(|e| match e {
        Event::Gone { agent, reward } => Some((*agent, *reward)),
        _ => None,
    });
    let gone = gone.unwrap();
    assert_eq!(gone.0.bot, BotId(1));
    // Losing its 5 points is the only change since its last turn.
    assert_eq!(
        gone.1,
        Reward {
            own: 0.0,
            team: -5.0,
            result: 0.0
        }
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::SetEnd { result, .. } if result.winner_team == Some(TeamId(0))
    )));
}

/// The commander takes wheat from the base above it once, and eats from it.
struct Eater;

impl ucbc_engine::Bot for Eater {
    fn step(&mut self, ctx: &mut ucbc_engine::StepCtx<'_>) -> StepResult {
        if ctx.tick == 0 {
            let take = json!({"type": "take", "dir": "up", "item": "wheat", "count": 20});
            ctx.act(&take).unwrap();
        }
        StepResult::ok()
    }
}

#[test]
fn a_unit_that_starves_at_the_end_of_a_tick_gets_no_result() {
    let eater = Seat::Team(Arc::new(|| TeamSpec::rust("eater", |_: &_| Eater)));
    let mut spec = env(vec![Seat::Policy, eater]);
    spec.max_ticks = 1000;
    let mut venv = VecEnv::new(registry(), vec![spec]).unwrap();
    let events = until(&mut venv, |e| matches!(e, Event::SetEnd { .. }));
    let detail = events.iter().find_map(|e| match e {
        Event::SetEnd { result, .. } => Some(result.detail.as_str()),
        _ => None,
    });
    assert_eq!(detail, Some("the other commander starved"));
    let gone = events.iter().find_map(|e| match e {
        Event::Gone { reward, .. } => Some((reward.result, reward.team)),
        _ => None,
    });
    assert_eq!(gone, Some((0.0, -5.0)));
}
