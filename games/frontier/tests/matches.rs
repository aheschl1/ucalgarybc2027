//! Full matches through the engine, with scripted bots defined here. The rules of each
//! action are tested in src/world.rs; these cover what needs the engine: step order,
//! observation, snapshots, the radio, and failures.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};
use ucbc_engine::{
    Bot, BotFailure, BotResourceLimit, EngineError, GameRegistry, MatchConfig, MatchRunner,
    MatchSpec, Reason, Replay, SpawnCtx, StepCtx, StepResult, TeamId, TeamSpec,
};
use ucbc_frontier::{Frontier, Kind, Observation, Snapshot};

const LIMITS: BotResourceLimit = BotResourceLimit::new(500, 1 << 30);

struct Script(fn(&mut StepCtx<'_>, &Observation) -> StepResult);

impl Bot for Script {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let seen = serde_json::from_value(ctx.query(&json!({"type": "observe"})).unwrap()).unwrap();
        (self.0)(ctx, &seen)
    }
}

fn team(name: &str, script: fn(&mut StepCtx<'_>, &Observation) -> StepResult) -> TeamSpec {
    TeamSpec::rust(name, move |_: &SpawnCtx| Script(script))
}

fn run(
    config: Value,
    sets: u32,
    ticks: u32,
    a: TeamSpec,
    b: TeamSpec,
) -> Result<Replay, EngineError> {
    let mut reg = GameRegistry::new();
    reg.register::<Frontier>();
    let config = MatchConfig::new("frontier", sets, 3, 2, LIMITS)
        .max_ticks(ticks)
        .game_config(config);
    Ok(
        MatchRunner::new(&reg, MatchSpec::new("t", config, vec![a, b]))?
            .run()?
            .replay,
    )
}

/// The initial state, then the state after each tick.
fn snapshots(replay: &Replay, set: usize) -> Vec<Snapshot> {
    let set = &replay.sets[set];
    std::iter::once(&set.initial_state)
        .chain(set.ticks.iter().map(|t| &t.state_after))
        .map(|s| serde_json::from_value(s.clone()).unwrap())
        .collect()
}

fn idle(_: &mut StepCtx<'_>, _: &Observation) -> StepResult {
    StepResult::ok()
}

/// The commander spawns every step it can; workers walk right when they can.
fn grow(ctx: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
    match seen.me.kind {
        Kind::Commander => drop(ctx.act(&json!({"type": "spawn"}))),
        Kind::Worker => drop(ctx.act(&json!({"type": "move", "dir": "right"}))),
    }
    StepResult::ok()
}

#[test]
fn observe_sees_a_square_clipped_to_the_map_with_flat_rows() {
    fn look(_: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        let tiles = BASE64.decode(&seen.tiles).unwrap();
        assert_eq!(tiles.len(), seen.width * seen.height);
        // The bases are 16 tiles from the edges of a 64 map, so 20 out clips there.
        let x = seen.me.pos.x;
        if seen.me.team == 0 {
            assert_eq!((seen.origin.x, seen.width), (0, x + 21));
        } else {
            assert_eq!((seen.origin.x, seen.width), (x - 20, 64 - (x - 20)));
        }
        assert_eq!(seen.height, 41);
        let me = [
            seen.me.id,
            seen.me.team.into(),
            0,
            x as u64,
            seen.me.pos.y as u64,
        ];
        assert!(seen.units.chunks(5).any(|u| u == me));
        // The base, with its storage next to the commander.
        let base = &seen.buildings[..7];
        assert_eq!(base[3..5], [seen.base.x as u64, seen.base.y as u64]);
        assert_eq!(seen.stores.len(), 1);
        assert_eq!(seen.stores[0].stored[0].count, 50);
        StepResult::ok()
    }
    run(
        json!({"size": 64, "vision": 20}),
        1,
        1,
        team("a", look),
        team("b", look),
    )
    .unwrap();
}

#[test]
fn bases_belong_to_teams_whichever_team_steps_first() {
    fn home(_: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        // Team 0's base is on the left, team 1's on the right.
        assert_eq!(seen.base.x < 32, seen.me.team == 0);
        StepResult::ok()
    }
    let replay = run(json!({"size": 64}), 2, 1, team("a", home), team("b", home)).unwrap();
    for set in 0..2 {
        let first = replay.sets[set].first_team;
        let s = snapshots(&replay, set);
        // Units step in spawn order: the first team's commander is the first unit.
        assert_eq!(s[0].units[1], u64::from(first.0));
    }
}

#[test]
fn snapshots_carry_the_map_once_then_deltas_and_keyframes() {
    let replay = run(
        json!({"size": 64, "keyframe": 4, "units": 3}),
        1,
        5,
        team("a", grow),
        team("b", idle),
    )
    .unwrap();
    let s = snapshots(&replay, 0);
    let map = s[0].map.as_ref().unwrap();
    assert_eq!(BASE64.decode(&map.tiles).unwrap().len(), 64 * 64);
    assert!(s[0].keyframe);
    assert_eq!((s[0].units.len() / 6, s[0].buildings.len() / 7), (2, 2));
    assert!(s[1..].iter().all(|x| x.map.is_none()));
    // After tick 0: only the new worker; the idle commander and the bases didn't change.
    assert!(!s[1].keyframe);
    assert_eq!(s[1].units.len() / 6, 1);
    assert_eq!(s[1].units[2], Kind::Worker as u64);
    assert!(s[1].buildings.is_empty());
    // After four ticks: a keyframe of everything, at the cap of 3.
    assert!(s[4].keyframe);
    assert_eq!(s[4].units.len() / 6, 3 + 1);
}

#[test]
fn what_is_said_is_heard_the_next_tick() {
    fn chat(ctx: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        match (seen.me.kind, ctx.tick) {
            (Kind::Commander, 0) => drop(ctx.act(&json!({"type": "spawn"}))),
            (Kind::Commander, 1) => {
                drop(ctx.act(&json!({"type": "say", "data": BASE64.encode("go")})))
            }
            (Kind::Commander, _) => assert!(seen.heard.is_empty()),
            (Kind::Worker, 1) => assert!(seen.heard.is_empty(), "said later this tick"),
            (Kind::Worker, 2) => {
                assert_eq!(seen.heard.len(), 1);
                assert_eq!(BASE64.decode(&seen.heard[0].data).unwrap(), b"go");
            }
            (Kind::Worker, _) => assert!(seen.heard.is_empty(), "heard once"),
        }
        StepResult::ok()
    }
    run(json!({"size": 64}), 1, 4, team("a", chat), team("b", chat)).unwrap();
}

#[test]
fn a_failing_commander_loses_and_a_failing_worker_leaves_its_tile() {
    fn fail_workers(ctx: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        match seen.me.kind {
            Kind::Commander => grow(ctx, seen),
            Kind::Worker => {
                StepResult::failed(BotFailure::exception("RuntimeError", "worker down"))
            }
        }
    }
    fn fail(_: &mut StepCtx<'_>, _: &Observation) -> StepResult {
        StepResult::failed(BotFailure::exception("RuntimeError", "commander down"))
    }
    let replay = run(
        json!({"size": 64}),
        1,
        4,
        team("a", fail_workers),
        team("b", fail),
    )
    .unwrap();
    let result = &replay.sets[0].result;
    assert_eq!(result.reason, Reason::Forfeit);
    assert_eq!(result.winner_team, Some(TeamId(0)));
    assert!(
        result.detail.contains("commander down"),
        "{}",
        result.detail
    );
    // The set ends during tick 0; its snapshot still follows the initial one.
    let ticks: Vec<u32> = snapshots(&replay, 0).iter().map(|s| s.tick).collect();
    assert_eq!(ticks, vec![0, 1]);

    let replay = run(
        json!({"size": 64, "units": 2}),
        1,
        4,
        team("a", fail_workers),
        team("b", idle),
    )
    .unwrap();
    let s = snapshots(&replay, 0);
    // The worker spawned in tick 0 fails in tick 1, which frees its tile and its place
    // under the cap: tick 2 spawns another on the same tile.
    let first = &s[1].units[..6];
    assert_eq!(s[2].removed, vec![first[0]]);
    let second = &s[3].units[..6];
    assert_ne!(second[0], first[0]);
    assert_eq!(second[3..5], first[3..5]);
}

#[test]
fn rejects_bad_config() {
    let bad = |config: Value| {
        run(config, 1, 1, team("a", idle), team("b", idle))
            .err()
            .unwrap()
            .to_string()
    };
    assert!(bad(json!({"size": 10})).contains("outside 64..=4096"));
    assert!(bad(json!({"colour": "red"})).contains("unknown field"));
    assert!(bad(json!({"size": 64, "vision": 65})).contains("vision 65 is over size 64"));
}
