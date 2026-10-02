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

fn go(game: &mut dyn DynGame, bot: BotId, x: u64, y: u64) -> Result<Value, ActionError> {
    game.apply_action(bot, &json!({"type": "move", "x": x, "y": y}))
}

fn item(game: &dyn DynGame, x: u64, y: u64) -> Value {
    game.handle_query(LAB0, &json!({"type": "item", "x": x, "y": y}))
        .unwrap()
}

/// Team 0's lab spawns a dino at (x, y) and ends its turn; the dino's id.
fn dino_at(game: &mut dyn DynGame, x: u64, y: u64) -> BotId {
    let spawned = spawn(game, LAB0, x, y).unwrap();
    game.end_step(LAB0);
    BotId(spawned["bot_id"].as_u64().unwrap())
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

#[test]
fn a_dino_moves_once_per_turn_diagonals_included() {
    let mut g = game();
    let dino = dino_at(g.as_mut(), 3, 7);
    assert_eq!(go(g.as_mut(), dino, 4, 6).unwrap(), json!({"x": 4, "y": 6}));
    let me = g.handle_query(dino, &json!({"type": "me"})).unwrap();
    assert_eq!(me["pos"], json!({"x": 4, "y": 6}));
    assert_eq!(item(g.as_ref(), 3, 7), Value::Null);
    assert_eq!(
        item(g.as_ref(), 4, 6),
        json!({"type": "dino", "team": 0, "level": 1})
    );
    refused(go(g.as_mut(), dino, 4, 7), "already moved");
    g.end_step(dino);
    go(g.as_mut(), dino, 4, 7).unwrap();
}

#[test]
fn a_move_must_be_in_range_onto_a_free_tile() {
    let mut g = game();
    let dino = dino_at(g.as_mut(), 3, 7);
    dino_at(g.as_mut(), 3, 8);
    refused(go(g.as_mut(), dino, 5, 7), "out of range");
    refused(go(g.as_mut(), dino, 2, 7), "not free"); // the lab
    refused(go(g.as_mut(), dino, 3, 8), "not free"); // another dino
    refused(go(g.as_mut(), dino, 3, 7), "not free"); // itself
    let negative = json!({"type": "move", "x": -1, "y": 7});
    assert!(matches!(
        g.apply_action(dino, &negative),
        Err(ActionError::Malformed(_))
    ));
    refused(go(g.as_mut(), LAB0, 0, 7), "only a dino");
    assert_eq!(item(g.as_ref(), 3, 7)["type"], "dino");
    // None of that used the turn; walk up to the wall at (5, 5), then the fossil
    // at (6, 7).
    go(g.as_mut(), dino, 4, 6).unwrap();
    g.end_step(dino);
    refused(go(g.as_mut(), dino, 5, 5), "not free");
    go(g.as_mut(), dino, 5, 6).unwrap();
    g.end_step(dino);
    refused(go(g.as_mut(), dino, 6, 7), "not free");
}

#[test]
fn a_move_stays_on_the_board() {
    let mut g = game();
    let spawned = spawn(g.as_mut(), LAB1, 15, 7).unwrap();
    let dino = BotId(spawned["bot_id"].as_u64().unwrap());
    refused(go(g.as_mut(), dino, 16, 7), "off the board");
}
