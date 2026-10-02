//! The game's rules, driven directly: one set, both labs, no runtimes.

use serde_json::{Value, json};
use ucbc_2027::Ucbc2027;
use ucbc_2027::rules::{BASE_INCOME, SPAWN_COST, START_BONES};
use ucbc_engine::{ActionError, BotFailure, BotId, DynGame, Game, SetSetup, TeamId};

const LAB0: BotId = BotId(0);
const LAB1: BotId = BotId(1);

/// Team 0 steps first. Its lab's top-left is (1, 7); team 1's is (13, 7).
fn game() -> Box<dyn DynGame> {
    let setup = SetSetup::new(0, vec![TeamId(0), TeamId(1)], 0, None);
    Box::new(Ucbc2027::create(&setup).unwrap())
}

fn spawn(game: &mut dyn DynGame, bot: BotId, x: u64, y: u64) -> Result<Value, ActionError> {
    game.apply_action(bot, &json!({"type": "spawn", "x": x, "y": y}))
}

fn bones(game: &dyn DynGame, bot: BotId) -> u64 {
    game.handle_query(bot, &json!({"type": "bones"}))
        .unwrap()
        .as_u64()
        .unwrap()
}

#[track_caller]
fn refused(result: Result<Value, ActionError>, why: &str) {
    match result {
        Err(ActionError::Invalid(message)) => assert!(message.contains(why), "{message}"),
        other => panic!("expected a refusal for {why:?}, got {other:?}"),
    }
}

#[test]
fn a_spawn_costs_bones_and_happens_once_per_turn() {
    let mut g = game();
    let start = u64::from(START_BONES);
    let cost = u64::from(SPAWN_COST);
    let spawned = spawn(g.as_mut(), LAB0, 0, 7).unwrap();
    assert_eq!(spawned, json!({"bot_id": 2, "at": {"x": 0, "y": 7}}));
    assert_eq!(bones(g.as_ref(), LAB0), start - cost);
    assert_eq!(bones(g.as_ref(), LAB1), start);
    refused(spawn(g.as_mut(), LAB0, 0, 6), "already spawned");
    g.end_step(LAB0);
    spawn(g.as_mut(), LAB0, 0, 6).unwrap();
    assert_eq!(bones(g.as_ref(), LAB0), start - 2 * cost);
}

#[test]
fn a_refused_spawn_spends_nothing() {
    let mut g = game();
    spawn(g.as_mut(), LAB0, 0, 7).unwrap();
    g.end_step(LAB0);
    let before = bones(g.as_ref(), LAB0);
    refused(spawn(g.as_mut(), LAB0, 0, 7), "not free");
    refused(spawn(g.as_mut(), LAB0, 1, 7), "next to the lab");
    refused(spawn(g.as_mut(), LAB0, 5, 7), "next to the lab");
    refused(spawn(g.as_mut(), LAB0, 99, 7), "next to the lab");
    refused(spawn(g.as_mut(), BotId(2), 0, 6), "only a lab");
    assert_eq!(bones(g.as_ref(), LAB0), before);
    // The turn was not used up either.
    spawn(g.as_mut(), LAB0, 0, 6).unwrap();
}

#[test]
fn the_next_bot_gets_a_fresh_turn_after_a_failure() {
    let mut g = game();
    spawn(g.as_mut(), LAB0, 0, 7).unwrap();
    g.bot_failed(LAB0, &BotFailure::exception("RuntimeError", "boom"));
    spawn(g.as_mut(), LAB1, 15, 7).unwrap();
}

#[test]
fn a_lab_runs_out_of_bones() {
    let mut g = game();
    let ring = (6..=9)
        .flat_map(|y| [(0, y), (3, y)])
        .chain([(1, 6), (2, 6), (1, 9), (2, 9)]);
    let mut spawned = 0;
    for (x, y) in ring {
        let before = bones(g.as_ref(), LAB0);
        let result = spawn(g.as_mut(), LAB0, x, y);
        g.end_step(LAB0);
        if result.is_err() {
            refused(result, "not enough bones");
            assert_eq!(bones(g.as_ref(), LAB0), before);
            break;
        }
        spawned += 1;
    }
    assert_eq!(spawned, START_BONES / SPAWN_COST);
}

#[test]
fn every_team_earns_bones_each_tick() {
    let mut g = game();
    g.end_tick();
    let after = u64::from(START_BONES + BASE_INCOME);
    assert_eq!(bones(g.as_ref(), LAB0), after);
    assert_eq!(bones(g.as_ref(), LAB1), after);
    assert_eq!(g.snapshot()["teams"][0]["bones"], json!(after));
}
