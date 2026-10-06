//! The game's rules, driven directly: one set, both labs, no runtimes.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use prost::Message;
use serde_json::{Value, json};
use ucbc_2027::rules::{
    ATTACK_COST, ATTACKS_PER_TURN, BASE_INCOME, DINO_HEALTH, DINO_MAX_HEALTH, EQUAL_LEVEL_DAMAGE,
    INCOME_PER_FOSSIL, LAB_HEALTH, MERGES_PER_TURN, SPAWN_COST, START_BONES, damage,
};
use ucbc_2027::{Coord, Ucbc2027, UnitView, proto};
use ucbc_engine::{ActionError, BotFailure, BotId, DynGame, EngineError, Game, SetSetup, TeamId};

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

fn grab(game: &mut dyn DynGame, bot: BotId, x: u64, y: u64) -> Result<Value, ActionError> {
    game.apply_action(bot, &json!({"type": "grab", "x": x, "y": y}))
}

fn put(game: &mut dyn DynGame, bot: BotId, x: u64, y: u64) -> Result<Value, ActionError> {
    game.apply_action(bot, &json!({"type": "drop", "x": x, "y": y}))
}

/// One move per turn along `path`.
fn walk(game: &mut dyn DynGame, bot: BotId, path: &[(u64, u64)]) {
    for &(x, y) in path {
        go(game, bot, x, y).unwrap();
        game.end_step(bot);
    }
}

fn held(game: &dyn DynGame, bot: BotId) -> Value {
    game.handle_query(bot, &json!({"type": "me"})).unwrap()["held"].clone()
}

/// A team 0 dino at (5, 6), next to the fossil at (6, 7).
fn by_fossil(game: &mut dyn DynGame) -> BotId {
    let dino = dino_at(game, 3, 7);
    walk(game, dino, &[(4, 6), (5, 6)]);
    dino
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

/// Asks `can_<kind>` first and checks the action agrees.
#[track_caller]
fn checked(
    game: &mut dyn DynGame,
    kind: &str,
    bot: BotId,
    target: BotId,
) -> Result<Value, ActionError> {
    let can = game
        .handle_query(
            bot,
            &json!({"type": format!("can_{kind}"), "bot_id": target.0}),
        )
        .unwrap();
    let result = game.apply_action(bot, &json!({"type": kind, "bot_id": target.0}));
    assert_eq!(
        can,
        json!(result.is_ok()),
        "can_{kind} disagrees: {result:?}"
    );
    result
}

#[track_caller]
fn merge(game: &mut dyn DynGame, bot: BotId, target: BotId) -> Result<Value, ActionError> {
    checked(game, "merge", bot, target)
}

#[track_caller]
fn attack(game: &mut dyn DynGame, bot: BotId, target: BotId) -> Result<Value, ActionError> {
    checked(game, "attack", bot, target)
}

/// A team 0 dino at (9, 6) facing a team 1 dino at (10, 6) that holds the fossil
/// from (9, 7).
fn face_off(game: &mut dyn DynGame) -> (BotId, BotId) {
    let ours = dino_at(game, 3, 7);
    let spawned = spawn(game, LAB1, 12, 6).unwrap();
    game.end_step(LAB1);
    let theirs = BotId(spawned["bot_id"].as_u64().unwrap());
    walk(game, theirs, &[(11, 6), (10, 6)]);
    grab(game, theirs, 9, 7).unwrap();
    let path: Vec<(u64, u64)> = (4..=9).map(|x| (x, 6)).collect();
    walk(game, ours, &path);
    (ours, theirs)
}

fn me(game: &dyn DynGame, bot: BotId) -> Value {
    game.handle_query(bot, &json!({"type": "me"})).unwrap()
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
        json!({"type": "dino", "id": dino.0, "team": 0, "level": 1})
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

#[test]
fn grab_and_put_are_free_and_hold_one_thing() {
    let mut g = game();
    let dino = by_fossil(g.as_mut());
    assert_eq!(
        grab(g.as_mut(), dino, 6, 7).unwrap(),
        json!({"type": "fossil"})
    );
    assert_eq!(held(g.as_ref(), dino), json!({"type": "fossil"}));
    assert_eq!(item(g.as_ref(), 6, 7), Value::Null);
    refused(grab(g.as_mut(), dino, 6, 7), "already holding");
    assert_eq!(
        put(g.as_mut(), dino, 6, 6).unwrap(),
        json!({"type": "placed"})
    );
    refused(put(g.as_mut(), dino, 6, 6), "holding nothing");
    assert_eq!(item(g.as_ref(), 6, 6), json!({"type": "fossil"}));
    // Again in the same turn, and the move is still unused.
    grab(g.as_mut(), dino, 6, 6).unwrap();
    put(g.as_mut(), dino, 6, 7).unwrap();
    go(g.as_mut(), dino, 5, 7).unwrap();
}

#[test]
fn grab_and_put_refuse_what_the_rules_forbid() {
    let mut g = game();
    let dino = by_fossil(g.as_mut());
    let other = dino_at(g.as_mut(), 3, 7);
    walk(g.as_mut(), other, &[(4, 7)]);
    refused(grab(g.as_mut(), dino, 7, 7), "out of range");
    refused(grab(g.as_mut(), dino, 6, 6), "nothing there");
    refused(grab(g.as_mut(), dino, 4, 7), "cannot carry");
    refused(grab(g.as_mut(), dino, 5, 6), "cannot carry"); // itself
    refused(grab(g.as_mut(), LAB0, 0, 7), "only a dino");
    assert_eq!(item(g.as_ref(), 4, 7)["type"], "dino");
    assert_eq!(item(g.as_ref(), 6, 7), json!({"type": "fossil"}));
    grab(g.as_mut(), dino, 6, 7).unwrap();
    refused(put(g.as_mut(), dino, 4, 7), "not free");
    refused(put(g.as_mut(), dino, 5, 5), "not free"); // a wall
    refused(put(g.as_mut(), dino, 5, 6), "not free"); // itself
    refused(put(g.as_mut(), dino, 7, 7), "out of range");
    refused(put(g.as_mut(), LAB0, 0, 7), "only a dino");
    assert_eq!(held(g.as_ref(), dino), json!({"type": "fossil"}));
}

#[test]
fn a_fossil_dropped_on_your_lab_raises_income() {
    let mut g = game();
    let dino = by_fossil(g.as_mut());
    grab(g.as_mut(), dino, 6, 7).unwrap();
    walk(g.as_mut(), dino, &[(4, 6), (3, 7)]);
    let deposited = put(g.as_mut(), dino, 2, 7).unwrap();
    assert_eq!(deposited, json!({"type": "deposited", "fossils": 1}));
    let fossils = |g: &dyn DynGame, bot| g.handle_query(bot, &json!({"type": "fossils"})).unwrap();
    assert_eq!(fossils(g.as_ref(), LAB0), json!(1));
    assert_eq!(fossils(g.as_ref(), LAB1), json!(0));
    assert_eq!(held(g.as_ref(), dino), Value::Null);
    let before = bones(g.as_ref(), LAB0);
    g.end_tick();
    let earned = bones(g.as_ref(), LAB0) - before;
    assert_eq!(earned, u64::from(BASE_INCOME + INCOME_PER_FOSSIL));
    let teams = &g.snapshot()["teams"];
    assert_eq!(teams[0]["fossils"], json!(1));
    assert_eq!(teams[1]["fossils"], json!(0));
}

#[test]
fn a_fossil_cannot_go_into_the_enemy_lab() {
    let mut g = game();
    let dino = by_fossil(g.as_mut());
    grab(g.as_mut(), dino, 6, 7).unwrap();
    let path: Vec<(u64, u64)> = (6..=12).map(|x| (x, 6)).collect();
    walk(g.as_mut(), dino, &path);
    refused(put(g.as_mut(), dino, 13, 7), "not your lab");
    assert_eq!(held(g.as_ref(), dino), json!({"type": "fossil"}));
    assert_eq!(g.snapshot()["teams"][1]["fossils"], json!(0));
}

#[test]
fn grab_stays_on_the_board() {
    let mut g = game();
    let spawned = spawn(g.as_mut(), LAB1, 15, 7).unwrap();
    let dino = BotId(spawned["bot_id"].as_u64().unwrap());
    refused(grab(g.as_mut(), dino, 16, 7), "off the board");
}

#[test]
fn a_failed_carrier_leaves_its_fossil_behind() {
    let mut g = game();
    let dino = by_fossil(g.as_mut());
    grab(g.as_mut(), dino, 6, 7).unwrap();
    g.bot_failed(dino, &BotFailure::exception("RuntimeError", "boom"));
    assert_eq!(item(g.as_ref(), 5, 6), json!({"type": "fossil"}));
    assert_eq!(item(g.as_ref(), 6, 7), Value::Null);
    let fossils = g.snapshot()["fossils"].as_array().unwrap().clone();
    assert!(fossils.contains(&json!({"x": 5, "y": 6})));
}

fn with_config(config: Value) -> Result<Ucbc2027, EngineError> {
    Ucbc2027::create(&SetSetup::new(
        0,
        vec![TeamId(0), TeamId(1)],
        0,
        Some(config),
    ))
}

#[test]
fn a_set_plays_on_the_map_in_game_config() {
    use proto::{Environment, Item};
    let tiles = ["LL....", "LL..LL", "...fLL"]
        .concat()
        .chars()
        .map(|ch| {
            let (environment, item) = match ch {
                'L' => (Environment::Lab, Item::None),
                'f' => (Environment::Empty, Item::Fossil),
                _ => (Environment::Empty, Item::None),
            };
            proto::Tile {
                environment: environment as i32,
                item: item as i32,
            }
        })
        .collect();
    let file = proto::Map {
        width: 6,
        height: 3,
        tiles,
    };
    let map = BASE64.encode(file.encode_to_vec());
    let g = with_config(json!({"map": map})).unwrap();
    let state = Game::snapshot(&g);
    assert_eq!(state.environment.len(), 3);
    assert_eq!(state.environment[0].len(), 6);
    assert_eq!(state.fossils, [Coord::new(3, 2)]);
    let lab1 = &state.units[1].unit;
    assert_eq!(
        *lab1,
        UnitView::Lab {
            id: 1,
            origin: Coord::new(4, 1),
            health: LAB_HEALTH
        }
    );
}

#[test]
fn a_bad_map_in_game_config_is_a_config_error() {
    for (config, why) in [
        (json!({"map": 3}), "expected a base64 string"),
        (json!({"map": "not base64!"}), "Invalid"),
        (
            json!({"map": BASE64.encode([0xff, 0xff])}),
            "not a map file",
        ),
    ] {
        match with_config(config) {
            Err(EngineError::Config(message)) => {
                assert!(message.starts_with("game_config.map: "), "{message}");
                assert!(message.contains(why), "{message:?} should say {why:?}");
            }
            Err(other) => panic!("expected a config error, got {other}"),
            Ok(_) => panic!("expected {why:?}"),
        }
    }
    // Without a map, the standard one.
    let standard = with_config(json!({})).unwrap();
    assert_eq!(Game::snapshot(&standard).environment.len(), 16);
}

/// The set's outcome if it ended now: (winner, detail).
fn verdict(game: &dyn DynGame) -> (Option<TeamId>, String) {
    let outcome = game.tick_limit(1000);
    (outcome.winner, outcome.detail)
}

#[test]
fn the_most_points_win_at_the_tick_limit() {
    let mut g = game();
    dino_at(g.as_mut(), 0, 7);
    assert_eq!(verdict(g.as_ref()), (Some(TeamId(1)), "most points".into()));
}

#[test]
fn even_points_go_to_the_most_fossils() {
    let mut g = game();
    let dino = by_fossil(g.as_mut());
    grab(g.as_mut(), dino, 6, 7).unwrap();
    walk(g.as_mut(), dino, &[(4, 6), (3, 7)]);
    put(g.as_mut(), dino, 2, 7).unwrap();
    spawn(g.as_mut(), LAB1, 15, 7).unwrap();
    assert_eq!(
        verdict(g.as_ref()),
        (Some(TeamId(0)), "most fossils".into())
    );
}

#[test]
fn then_to_the_highest_level_dino() {
    let mut g = game();
    dino_at(g.as_mut(), 0, 7);
    let spawned = spawn(g.as_mut(), LAB1, 15, 7).unwrap();
    let dino = BotId(spawned["bot_id"].as_u64().unwrap());
    g.bot_failed(dino, &BotFailure::exception("RuntimeError", "boom"));
    let detail = "highest level dino".to_string();
    assert_eq!(verdict(g.as_ref()), (Some(TeamId(0)), detail));
}

#[test]
fn a_full_tie_is_a_coin_toss_on_the_seed() {
    let toss = |seed| {
        let setup = SetSetup::new(0, vec![TeamId(0), TeamId(1)], seed, None);
        verdict(&Ucbc2027::create(&setup).unwrap())
    };
    assert_eq!(toss(0), (Some(TeamId(0)), "coin toss".into()));
    assert_eq!(toss(1), (Some(TeamId(1)), "coin toss".into()));
}

#[test]
fn a_merge_adds_levels_and_takes_the_target_tile() {
    let mut g = game();
    let dino = dino_at(g.as_mut(), 3, 7);
    let other = dino_at(g.as_mut(), 3, 8);
    assert_eq!(item(g.as_ref(), 3, 8)["id"], json!(other.0));
    let merged = merge(g.as_mut(), dino, other).unwrap();
    let health = 2 * DINO_HEALTH;
    assert_eq!(
        merged,
        json!({"type": "dino", "id": dino.0, "pos": {"x": 3, "y": 8}, "level": 2, "health": health, "held": null})
    );
    assert_eq!(item(g.as_ref(), 3, 7), Value::Null);
    assert_eq!(
        item(g.as_ref(), 3, 8),
        json!({"type": "dino", "id": dino.0, "team": 0, "level": 2})
    );
    assert!(g.team_of(other).is_none());
    let ids: Vec<Value> = g.snapshot()["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["id"].clone())
        .collect();
    assert_eq!(ids, [json!(0), json!(1), json!(dino.0)]);
    // The move is still unused.
    go(g.as_mut(), dino, 4, 8).unwrap();
}

#[test]
fn merges_per_turn_are_limited() {
    let mut g = game();
    let dino = dino_at(g.as_mut(), 0, 8);
    let others: Vec<BotId> = (0..=MERGES_PER_TURN)
        .map(|i| dino_at(g.as_mut(), 0, 7 - u64::from(i)))
        .collect();
    for &other in &others[..MERGES_PER_TURN as usize] {
        merge(g.as_mut(), dino, other).unwrap();
    }
    let last = *others.last().unwrap();
    refused(merge(g.as_mut(), dino, last), "already merged");
    g.end_step(dino);
    merge(g.as_mut(), dino, last).unwrap();
}

#[test]
fn a_merge_refuses_what_the_rules_forbid() {
    let mut g = game();
    let dino = dino_at(g.as_mut(), 3, 7);
    let far = dino_at(g.as_mut(), 3, 9);
    let spawned = spawn(g.as_mut(), LAB1, 12, 7).unwrap();
    let enemy = BotId(spawned["bot_id"].as_u64().unwrap());
    refused(merge(g.as_mut(), LAB0, dino), "only a dino");
    refused(merge(g.as_mut(), dino, dino), "itself");
    refused(merge(g.as_mut(), dino, BotId(99)), "no such bot");
    refused(merge(g.as_mut(), dino, LAB0), "only merge with a dino");
    refused(merge(g.as_mut(), dino, LAB1), "not your team");
    refused(merge(g.as_mut(), dino, enemy), "not your team");
    refused(merge(g.as_mut(), dino, far), "out of range");
    assert_eq!(me(g.as_ref(), dino)["level"], json!(1));
    assert_eq!(item(g.as_ref(), 3, 9)["id"], json!(far.0));
    // None of that used the merge.
    walk(g.as_mut(), far, &[(3, 8)]);
    merge(g.as_mut(), dino, far).unwrap();
}

#[test]
fn merged_health_is_capped() {
    let mut g = game();
    for _ in 0..SPAWN_COST {
        g.end_tick();
    }
    let ring = [(0, 9), (0, 8), (0, 7), (0, 6), (1, 6), (2, 6)];
    let dinos: Vec<BotId> = ring
        .iter()
        .map(|&(x, y)| dino_at(g.as_mut(), x, y))
        .collect();
    for &other in &dinos[1..] {
        merge(g.as_mut(), dinos[0], other).unwrap();
        g.end_step(dinos[0]);
    }
    let me = me(g.as_ref(), dinos[0]);
    const { assert!(DINO_HEALTH * 6 > DINO_MAX_HEALTH) };
    assert_eq!(me["level"], json!(6));
    assert_eq!(me["health"], json!(DINO_MAX_HEALTH));
    assert_eq!(me["pos"], json!({"x": 2, "y": 6}));
}

#[test]
fn a_merge_takes_the_target_load_and_leaves_its_own() {
    // The target holds; the merger holds nothing.
    let mut g = game();
    let carrier = by_fossil(g.as_mut());
    grab(g.as_mut(), carrier, 6, 7).unwrap();
    let dino = dino_at(g.as_mut(), 3, 7);
    walk(g.as_mut(), dino, &[(4, 6)]);
    merge(g.as_mut(), dino, carrier).unwrap();
    assert_eq!(held(g.as_ref(), dino), json!({"type": "fossil"}));
    assert_eq!(item(g.as_ref(), 4, 6), Value::Null);

    // The merger holds; the target holds nothing.
    let mut g = game();
    let carrier = by_fossil(g.as_mut());
    grab(g.as_mut(), carrier, 6, 7).unwrap();
    let dino = dino_at(g.as_mut(), 3, 7);
    walk(g.as_mut(), dino, &[(4, 6)]);
    merge(g.as_mut(), carrier, dino).unwrap();
    assert_eq!(held(g.as_ref(), carrier), json!({"type": "fossil"}));
    assert_eq!(item(g.as_ref(), 5, 6), Value::Null);

    // Both hold: the merger's goes down where it stood.
    let mut g = game();
    let first = by_fossil(g.as_mut());
    grab(g.as_mut(), first, 6, 7).unwrap();
    walk(g.as_mut(), first, &[(6, 6), (7, 6), (8, 6)]);
    put(g.as_mut(), first, 7, 7).unwrap();
    grab(g.as_mut(), first, 9, 7).unwrap();
    let second = dino_at(g.as_mut(), 3, 7);
    walk(g.as_mut(), second, &[(4, 6), (5, 6), (6, 6)]);
    grab(g.as_mut(), second, 7, 7).unwrap();
    walk(g.as_mut(), second, &[(7, 6)]);
    merge(g.as_mut(), second, first).unwrap();
    assert_eq!(me(g.as_ref(), second)["pos"], json!({"x": 8, "y": 6}));
    assert_eq!(held(g.as_ref(), second), json!({"type": "fossil"}));
    assert_eq!(item(g.as_ref(), 7, 6), json!({"type": "fossil"}));
}

#[test]
fn an_absorbed_dino_does_not_step_again() {
    let mut g = game();
    let dino = dino_at(g.as_mut(), 3, 7);
    let other = dino_at(g.as_mut(), 3, 8);
    merge(g.as_mut(), dino, other).unwrap();
    assert!(g.team_of(other).is_none());
    assert!(!g.schedule().contains(&other));
}

#[test]
fn damage_is_the_level_gap_squared_and_half_that_upward() {
    assert_eq!(damage(1, 1), EQUAL_LEVEL_DAMAGE);
    assert_eq!(damage(3, 3), EQUAL_LEVEL_DAMAGE);
    assert_eq!(damage(3, 1), 4);
    assert_eq!(damage(4, 1), 9);
    assert_eq!(damage(1, 3), 2);
    assert_eq!(damage(1, 4), 4);
    // Never nothing.
    assert_eq!(damage(2, 1), 1);
    assert_eq!(damage(1, 2), 1);
}

#[test]
fn an_attack_costs_bones_and_happens_a_limited_number_of_times_per_turn() {
    let mut g = game();
    let (ours, theirs) = face_off(g.as_mut());
    let before = bones(g.as_ref(), LAB0);
    for i in 1..=ATTACKS_PER_TURN {
        assert_eq!(attack(g.as_mut(), ours, theirs).unwrap(), Value::Null);
        let health = DINO_HEALTH - i * EQUAL_LEVEL_DAMAGE;
        assert_eq!(me(g.as_ref(), theirs)["health"], json!(health));
    }
    let spent = u64::from(ATTACKS_PER_TURN * ATTACK_COST);
    assert_eq!(bones(g.as_ref(), LAB0), before - spent);
    refused(attack(g.as_mut(), ours, theirs), "already attacked");
    // The move is still unused.
    go(g.as_mut(), ours, 9, 5).unwrap();
    g.end_step(ours);
    attack(g.as_mut(), ours, theirs).unwrap();
}

#[test]
fn an_attack_refuses_what_the_rules_forbid() {
    let mut g = game();
    let (ours, theirs) = face_off(g.as_mut());
    let friend = dino_at(g.as_mut(), 3, 7);
    walk(
        g.as_mut(),
        friend,
        &[(4, 6), (5, 6), (6, 6), (7, 6), (8, 6)],
    );
    refused(attack(g.as_mut(), LAB0, theirs), "only a dino");
    refused(attack(g.as_mut(), ours, ours), "own team");
    refused(attack(g.as_mut(), ours, friend), "own team");
    refused(attack(g.as_mut(), ours, BotId(99)), "no such bot");
    refused(attack(g.as_mut(), ours, LAB1), "only attack a dino");
    refused(attack(g.as_mut(), friend, theirs), "out of range");
    assert_eq!(me(g.as_ref(), theirs)["health"], json!(DINO_HEALTH));
    // Spend the rest of team 0's bones on spawns.
    let ring = [(0, 6), (0, 7), (0, 8), (0, 9), (1, 6), (2, 6)];
    for &(x, y) in &ring[..(bones(g.as_ref(), LAB0) / u64::from(SPAWN_COST)) as usize] {
        dino_at(g.as_mut(), x, y);
    }
    assert!(bones(g.as_ref(), LAB0) < u64::from(ATTACK_COST));
    refused(attack(g.as_mut(), ours, theirs), "not enough bones");
    assert_eq!(me(g.as_ref(), theirs)["health"], json!(DINO_HEALTH));
    // None of that used the attack.
    for _ in 0..ATTACK_COST / BASE_INCOME {
        g.end_tick();
    }
    attack(g.as_mut(), ours, theirs).unwrap();
}

#[test]
fn a_dino_with_no_health_dies_and_drops_its_load() {
    let mut g = game();
    let (ours, theirs) = face_off(g.as_mut());
    let hits = DINO_HEALTH.div_ceil(EQUAL_LEVEL_DAMAGE);
    for _ in 1..hits {
        attack(g.as_mut(), ours, theirs).unwrap();
        g.end_step(ours);
    }
    attack(g.as_mut(), ours, theirs).unwrap();
    assert!(g.team_of(theirs).is_none());
    assert!(!g.schedule().contains(&theirs));
    assert_eq!(item(g.as_ref(), 10, 6), json!({"type": "fossil"}));
    g.end_step(ours);
    refused(attack(g.as_mut(), ours, theirs), "no such bot");
}

#[test]
fn a_bigger_dino_hits_harder() {
    let mut g = game();
    let (ours, theirs) = face_off(g.as_mut());
    let friend = dino_at(g.as_mut(), 3, 7);
    walk(
        g.as_mut(),
        friend,
        &[(4, 6), (5, 6), (6, 6), (7, 6), (8, 6)],
    );
    merge(g.as_mut(), friend, ours).unwrap();
    attack(g.as_mut(), friend, theirs).unwrap();
    let health = DINO_HEALTH - damage(2, 1);
    assert_eq!(me(g.as_ref(), theirs)["health"], json!(health));
}
