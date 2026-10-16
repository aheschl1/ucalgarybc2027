//! Full matches through the engine on small maps, with scripted bots defined here.

use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};
use ucbc_engine::{
    ActionError, Bot, BotFailure, BotResourceLimit, EngineError, GameRegistry, MatchConfig,
    MatchRunner, MatchSpec, Reason, Replay, SpawnCtx, StepCtx, StepResult, TeamId, TeamSpec,
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
fn the_commander_spawns_up_to_the_cap() {
    let replay = run(
        json!({"size": 64, "units": 4}),
        1,
        6,
        team("a", grow),
        team("b", idle),
    )
    .unwrap();
    let s = snapshots(&replay, 0);
    let count = |team: u32| {
        let units = s.iter().flat_map(|x| &x.units).filter(|u| u.team == team);
        units.map(|u| u.id).collect::<BTreeSet<_>>().len()
    };
    assert_eq!(count(0), 4);
    assert_eq!(count(1), 1);
    assert_eq!(replay.sets[0].result.reason, Reason::Draw);
    assert_eq!(s.last().unwrap().tick, 6);
}

#[test]
fn spawn_and_move_refuse_what_the_rules_forbid() {
    fn probe(ctx: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        let refused = |r: Result<Value, ActionError>, why: &str| match r {
            Err(ActionError::Invalid(msg)) => assert!(msg.contains(why), "{msg}"),
            other => panic!("expected a refusal, got {other:?}"),
        };
        if seen.me.kind == Kind::Commander && ctx.tick == 0 {
            ctx.act(&json!({"type": "spawn"})).unwrap();
            refused(ctx.act(&json!({"type": "spawn"})), "already spawned");
            // Up is the base itself.
            refused(ctx.act(&json!({"type": "move", "dir": "up"})), "blocked");
            ctx.act(&json!({"type": "move", "dir": "down"})).unwrap();
            refused(
                ctx.act(&json!({"type": "move", "dir": "down"})),
                "already moved",
            );
        } else if seen.me.kind == Kind::Commander {
            refused(ctx.act(&json!({"type": "spawn"})), "next to the base");
        } else {
            refused(ctx.act(&json!({"type": "spawn"})), "only a commander");
        }
        StepResult::ok()
    }
    run(json!({"size": 64}), 1, 2, team("a", probe), team("b", idle)).unwrap();
}

#[test]
fn observe_sees_a_square_clipped_to_the_map() {
    fn look(_: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        let tiles = BASE64.decode(&seen.tiles).unwrap();
        assert_eq!(tiles.len(), seen.width * seen.height);
        // Team 0's base is 16 tiles from the left edge of a 64 map, so 20 out clips there.
        assert_eq!(seen.origin.x, 0);
        assert_eq!(seen.width, seen.me.pos.x + 21);
        assert_eq!(seen.height, 41);
        let at = |x: usize, y: usize| tiles[(y - seen.origin.y) * seen.width + (x - seen.origin.x)];
        assert_eq!(at(seen.base.x, seen.base.y), 5);
        assert!(seen.units.contains(&seen.me));
        StepResult::ok()
    }
    run(
        json!({"size": 64, "vision": 20}),
        1,
        1,
        team("a", look),
        team("b", idle),
    )
    .unwrap();
}

#[test]
fn bases_are_indexed_by_team_whichever_team_steps_first() {
    fn home(_: &mut StepCtx<'_>, seen: &Observation) -> StepResult {
        // Team 0's base is on the left, team 1's on the right.
        assert_eq!(seen.base.x < 32, seen.me.team == 0);
        StepResult::ok()
    }
    let replay = run(json!({"size": 64}), 2, 1, team("a", home), team("b", home)).unwrap();
    for set in 0..2 {
        let s = snapshots(&replay, set);
        let bases = &s[0].map.as_ref().unwrap().bases;
        for unit in &s[0].units {
            assert_eq!(unit.pos.x, bases[unit.team as usize].x);
        }
    }
}

#[test]
fn snapshots_carry_the_map_once_then_deltas_and_keyframes() {
    let replay = run(
        json!({"size": 64, "keyframe": 4}),
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
    assert_eq!(s[0].units.len(), 2);
    assert!(s[1..].iter().all(|x| x.map.is_none()));
    // After tick 0: the new worker only; b's idle commander didn't change.
    assert!(!s[1].keyframe);
    assert_eq!(s[1].units.len(), 1);
    assert_eq!(s[1].units[0].kind, Kind::Worker);
    // After four ticks: a keyframe of every unit.
    assert!(s[4].keyframe);
    assert_eq!(s[4].units.len(), 2 + 4);
}

#[test]
fn a_failing_commander_loses_and_a_failing_worker_is_removed() {
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
    let first = s[1].units[0];
    assert_eq!(s[2].removed, vec![first.id]);
    let second = s[3].units[0];
    assert_ne!(second.id, first.id);
    assert_eq!(second.pos, first.pos);
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
