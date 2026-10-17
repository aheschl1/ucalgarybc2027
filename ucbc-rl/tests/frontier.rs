use serde_json::{Value, json};
use ucbc_engine::{BotId, DynGame, Game, SetSetup, TeamId};
use ucbc_frontier::{Building, Frontier, Item};
use ucbc_rl::frontier::{Codec, Context, HEADS, VECTOR, VERBS, plane};

const CODEC: Codec = Codec {
    vision: 16,
    max_ticks: 1000,
};

fn game(config: Value) -> Frontier {
    let setup = SetSetup::new(0, vec![TeamId(0), TeamId(1)], 3, Some(config));
    Frontier::create(&setup).unwrap()
}

struct Encoded {
    grid: Vec<u8>,
    vector: Vec<f32>,
    at: Context,
}

fn encode(game: &Frontier, bot: u64) -> Encoded {
    let obs = DynGame::handle_query(game, BotId(bot), &json!({"type": "observe"})).unwrap();
    let (mut grid, mut vector) = (vec![0; CODEC.grid_len()], vec![0.0; VECTOR]);
    let at = CODEC.encode(&obs, &mut grid, &mut vector).unwrap();
    Encoded { grid, vector, at }
}

fn verb(name: &str) -> usize {
    VERBS.iter().position(|&v| v == name).unwrap()
}

fn wood() -> usize {
    Item::ALL.iter().position(|&i| i == Item::Wood).unwrap()
}

/// The cell at `(dx, dy)` from the unit, in plane `p`.
fn cell(e: &Encoded, p: usize, dx: isize, dy: isize) -> u8 {
    let side = CODEC.side() as isize;
    let v = CODEC.vision as isize;
    e.grid[p * (side * side) as usize + ((v + dy) * side + v + dx) as usize]
}

fn same(a: &Encoded, b: &Encoded) {
    assert!(!a.at.mirror && b.at.mirror);
    assert_eq!(a.grid, b.grid);
    assert_eq!(a.vector, b.vector);
}

#[test]
fn mirrored_commanders_encode_alike() {
    for config in [json!({"size": 64}), json!({"size": 64, "flat": true})] {
        let game = game(config);
        same(&encode(&game, 0), &encode(&game, 1));
    }
}

#[test]
fn mirrored_play_stays_mirrored() {
    let mut game = game(json!({"size": 64, "flat": true}));
    // Spawn; take wood from the base above, step left; drop some below, step left; again,
    // down.
    let steps = [
        [0, verb("spawn"), 0, 0, 0, 0, 0, 0, 0],
        [3, verb("take"), 0, wood(), 1, 0, 0, 0, 0],
        [3, verb("put"), 1, wood(), 0, 0, 0, 0, 0],
        [2, verb("put"), 1, wood(), 0, 0, 0, 0, 0],
    ];
    for ids in steps {
        for bot in [0, 1] {
            for action in encode(&game, bot).at.decode(&ids) {
                DynGame::apply_action(&mut game, BotId(bot), &action).unwrap();
            }
            Game::end_step(&mut game, BotId(bot));
        }
        same(&encode(&game, 0), &encode(&game, 1));
    }
    let e = encode(&game, 0);
    assert_eq!(e.vector[6..8], [2.0 / 64.0, -2.0 / 64.0]);
    assert_eq!(cell(&e, plane::PILE, 1, 0), 1);
    assert_eq!(cell(&e, plane::PILE, 0, 0), 1);
}

#[test]
fn a_commander_starts_below_its_base() {
    let e = encode(&game(json!({"size": 64, "flat": true})), 0);
    let base = Building::ALL
        .iter()
        .position(|&b| b == Building::Base)
        .unwrap();
    assert_eq!(cell(&e, plane::UNIT, 0, 0), 1);
    assert_eq!(cell(&e, plane::UNIT_HP, 0, 0), 100);
    assert_eq!(cell(&e, plane::BUILDING, 0, -1), base as u8 + 1);
    assert_eq!(cell(&e, plane::BUILDING_STATE, 0, -1), 1);
    assert_eq!(cell(&e, plane::TILE, 0, -1), 1);
    assert_eq!(
        e.vector[..9],
        [1.0, 1.0, 1.0, 0.25, 33.0 / 64.0, 1.0, 0.0, -1.0 / 64.0, 0.0]
    );
}

#[test]
fn team_1_acts_mirrored_and_the_game_takes_it() {
    let mut game = game(json!({"size": 64, "flat": true}));
    let e = encode(&game, 1);
    let ids = |mv, verb, target| [mv, verb, 0, 0, 0, 0, 0, target, 0];
    // Left in its frame is right on the map; an offset of +1 is one tile further left.
    assert_eq!(
        e.at.decode(&ids(3, verb("attack"), 7 * 15 + 8)),
        [
            json!({"type": "attack", "x": e.at.x - 1, "y": e.at.y}),
            json!({"type": "move", "dir": "right"}),
        ]
    );
    assert_eq!(e.at.decode(&ids(0, verb("none"), 0)), Vec::<Value>::new());

    let spawn = e.at.decode(&ids(0, verb("spawn"), 0));
    assert_eq!(spawn, [json!({"type": "spawn"})]);
    DynGame::apply_action(&mut game, BotId(1), &spawn[0]).unwrap();
}

#[test]
fn verbs_name_their_items_buildings_and_recipes() {
    let at = Context {
        mirror: false,
        x: 5,
        y: 5,
        size: 64,
    };
    let put = [0, verb("put"), 1, wood(), 2, 0, 0, 0, 0];
    assert_eq!(
        at.decode(&put),
        [json!({"type": "put", "dir": "down", "item": "wood", "count": 20})]
    );
    let mut build = [2, verb("build"), 2, 0, 0, 1, 0, 0, 0];
    let (built, moved) = (
        json!({"type": "build", "dir": "left", "building": Building::ALL[1]}),
        json!({"type": "move", "dir": "down"}),
    );
    assert_eq!(at.decode(&build), [built.clone(), moved.clone()]);
    build[8] = 1;
    assert_eq!(at.decode(&build), [moved, built]);
    // Attacks off either edge of the map are left out.
    let attack = |target| [0, verb("attack"), 0, 0, 0, 0, 0, target, 0];
    assert_eq!(
        Context { x: 0, ..at }.decode(&attack(0)),
        Vec::<Value>::new()
    );
    let edge = Context { x: 63, y: 63, ..at };
    assert_eq!(edge.decode(&attack(15 * 15 - 1)), Vec::<Value>::new());
}

#[test]
fn every_head_decodes_to_something_the_game_parses() {
    let at = Context {
        mirror: false,
        x: 30,
        y: 30,
        size: 64,
    };
    for verb in 0..HEADS[1] {
        for i in 0..HEADS.iter().copied().max().unwrap() {
            let mut ids = HEADS.map(|n| i % n);
            ids[1] = verb;
            for action in at.decode(&ids) {
                serde_json::from_value::<ucbc_frontier::Action>(action.clone())
                    .unwrap_or_else(|e| panic!("{action}: {e}"));
            }
        }
    }
}

#[test]
fn moving_first_acts_from_the_new_tile() {
    let mut game = game(json!({"size": 64, "flat": true}));
    let mut play = |bot: u64, ids: [usize; 9]| {
        let results: Vec<bool> = encode(&game, bot)
            .at
            .decode(&ids)
            .iter()
            .map(|a| DynGame::apply_action(&mut game, BotId(bot), a).is_ok())
            .collect();
        Game::end_step(&mut game, BotId(bot));
        results
    };
    for bot in [0, 1] {
        // Step left, away from the base; then back right and take from it, above.
        assert_eq!(play(bot, [3, 0, 0, 0, 0, 0, 0, 0, 0]), [true]);
        let back_and_take = [4, verb("take"), 0, wood(), 0, 0, 0, 0, 1];
        assert_eq!(play(bot, back_and_take), [true, true]);
        // Verb first, the take happens beside the base, then the step away.
        let take_and_leave = [3, verb("take"), 0, wood(), 0, 0, 0, 0, 0];
        assert_eq!(play(bot, take_and_leave), [true, true]);
    }
}
