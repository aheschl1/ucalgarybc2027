//! Full matches through the engine, with minimal bots defined here.

use serde_json::{Value, json};
use ucbc_2027::Ucbc2027;
use ucbc_engine::{
    Bot, BotFailure, BotResourceLimit, GameRegistry, MatchConfig, MatchRunner, MatchSpec, Reason,
    Replay, SpawnCtx, StepCtx, StepResult, TeamId, TeamSpec,
};

const LIMITS: BotResourceLimit = BotResourceLimit::new(500, 1 << 30);
const TICKS: u32 = 5;

fn me(ctx: &mut StepCtx<'_>) -> Value {
    ctx.query(&json!({"type": "me"})).unwrap()
}

/// The tile left of a lab's top-left corner: on the ring around it.
fn beside(lab: &Value) -> (u64, u64) {
    let origin = &lab["origin"];
    (
        origin["x"].as_u64().unwrap() - 1,
        origin["y"].as_u64().unwrap(),
    )
}

/// Checks that its lab stands on lab tiles of its own team, then does nothing.
#[derive(Default)]
struct Noop;

impl Bot for Noop {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let lab = me(ctx);
        assert_eq!(lab["type"], "lab");
        let at = json!({"type": "environment", "x": lab["origin"]["x"], "y": lab["origin"]["y"]});
        assert_eq!(
            ctx.query(&at).unwrap(),
            json!({"type": "lab", "team": ctx.team.id.0})
        );
        assert!(
            ctx.query(&json!({"type": "item", "x": 99, "y": 0}))
                .is_err()
        );
        assert_eq!(ctx.act(&json!({"type": "noop"})).unwrap(), json!(null));
        StepResult::ok()
    }
}

/// On its first step a lab spawns a dino beside it, and tries the same tile again on
/// tick 2; a dino tries to spawn and is refused. With `FAIL`, dinos fail on their
/// first step, which frees the tile.
#[derive(Default)]
struct Spawner<const FAIL: bool>;

impl<const FAIL: bool> Bot for Spawner<FAIL> {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let unit = me(ctx);
        let spawn = |(x, y): (u64, u64)| json!({"type": "spawn", "x": x, "y": y});
        if unit["type"] == "dino" {
            if FAIL {
                return StepResult::failed(BotFailure::exception("RuntimeError", "dino down"));
            }
            assert!(ctx.act(&spawn((0, 0))).is_err());
        } else if ctx.tick == 0 {
            let (x, y) = beside(&unit);
            // Far away, then on the lab itself.
            assert!(ctx.act(&spawn((x + 6, y))).is_err());
            assert!(ctx.act(&spawn((x + 1, y))).is_err());
            let spawned = ctx.act(&spawn((x, y))).unwrap();
            assert_eq!(spawned["at"], json!({"x": x, "y": y}));
            assert!(ctx.act(&spawn((x, y))).is_err());
        } else if ctx.tick == 2 {
            assert_eq!(ctx.act(&spawn(beside(&unit))).is_ok(), FAIL);
        }
        StepResult::ok()
    }
}

#[derive(Default)]
struct Failing;

impl Bot for Failing {
    fn step(&mut self, _ctx: &mut StepCtx<'_>) -> StepResult {
        StepResult::failed(BotFailure::exception("RuntimeError", "I give up"))
    }
}

fn team<B: Bot + Default + 'static>(name: &str) -> TeamSpec {
    TeamSpec::rust(name, |_: &SpawnCtx| B::default())
}

fn run(teams: Vec<TeamSpec>, sets: u32) -> Result<Replay, ucbc_engine::EngineError> {
    let mut reg = GameRegistry::new();
    reg.register::<Ucbc2027>();
    let config = MatchConfig::new("ucbc2027", sets, 0, teams.len() as u32, LIMITS).max_ticks(TICKS);
    let spec = MatchSpec::new("t", config, teams);
    Ok(MatchRunner::new(&reg, spec)?.run()?.replay)
}

fn steppers(replay: &Replay, set: usize) -> Vec<u32> {
    replay.sets[set]
        .ticks
        .iter()
        .flat_map(|t| t.steps.iter().map(|s| s.team.0))
        .collect()
}

fn dinos(state: &Value) -> Vec<&Value> {
    state["units"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|u| u["type"] == "dino")
        .collect()
}

#[test]
fn sets_run_to_the_tick_limit_and_end_on_a_coin_toss_when_even() {
    let replay = run(vec![team::<Noop>("a"), team::<Noop>("b")], 2).unwrap();
    for set in &replay.sets {
        assert_eq!(set.result.reason, Reason::Win);
        assert_eq!(set.result.detail, "coin toss");
        assert_eq!(set.result.ticks, TICKS);
        let ticks: Vec<u64> = set
            .ticks
            .iter()
            .map(|t| t.state_after["tick"].as_u64().unwrap())
            .collect();
        assert_eq!(ticks, vec![1, 2, 3, 4, 5]);
    }
    assert_eq!(steppers(&replay, 0)[..2], [0, 1]);
    assert_eq!(steppers(&replay, 1)[..2], [1, 0]);
}

#[test]
fn the_initial_state_has_both_labs_and_the_fossils() {
    let replay = run(vec![team::<Noop>("a"), team::<Noop>("b")], 1).unwrap();
    let state = &replay.sets[0].initial_state;
    let labs: Vec<(&Value, &Value)> = state["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| (&u["team"], &u["type"]))
        .collect();
    assert_eq!(
        labs,
        [(&json!(0), &json!("lab")), (&json!(1), &json!("lab"))]
    );
    assert_eq!(state["fossils"].as_array().unwrap().len(), 6);
    assert_eq!(state["teams"][0], state["teams"][1]);
}

#[test]
fn a_failed_bot_sits_out_the_set() {
    let replay = run(vec![team::<Failing>("a"), team::<Noop>("b")], 1).unwrap();
    let steppers = steppers(&replay, 0);
    assert_eq!(steppers.iter().filter(|&&t| t == 0).count(), 1);
    assert_eq!(steppers.iter().filter(|&&t| t == 1).count(), TICKS as usize);
    assert_eq!(replay.sets[0].ticks[0].steps[0].team, TeamId(0));
}

#[test]
fn a_lab_spawns_a_dino_that_steps_from_the_next_tick() {
    let replay = run(
        vec![team::<Spawner<false>>("a"), team::<Spawner<false>>("b")],
        1,
    )
    .unwrap();
    let set = &replay.sets[0];
    assert_eq!(set.ticks[0].steps.len(), 2);
    let order: Vec<u64> = set.ticks[1].steps.iter().map(|s| s.bot.0).collect();
    assert_eq!(order, vec![0, 1, 2, 3]);
    let state = &set.ticks[0].state_after;
    let spawned = dinos(state);
    assert_eq!(spawned.len(), 2);
    assert_eq!(spawned[0]["team"], 0);
    assert_eq!(spawned[0]["level"], 1);
    assert_eq!(spawned[0]["held"], Value::Null);
    let (x, y) = beside(&state["units"][0]);
    assert_eq!(spawned[0]["pos"], json!({"x": x, "y": y}));
    assert!(
        set.ticks
            .iter()
            .flat_map(|t| &t.steps)
            .all(|s| s.failure.is_none())
    );
}

#[test]
fn a_failed_dino_leaves_the_board() {
    let replay = run(
        vec![team::<Spawner<true>>("a"), team::<Spawner<false>>("b")],
        1,
    )
    .unwrap();
    let set = &replay.sets[0];
    assert_eq!(dinos(&set.ticks[0].state_after).len(), 2);
    let after = dinos(&set.ticks[1].state_after);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0]["team"], 1);
    // Team 0's lab spawned on the freed tile at tick 2; the bots assert the rest.
    assert_eq!(dinos(&set.ticks[2].state_after).len(), 2);
    assert!(set.ticks[2].steps.iter().all(|s| s.failure.is_none()));
}

#[test]
fn needs_exactly_two_teams() {
    let teams = vec![team::<Noop>("a"), team::<Noop>("b"), team::<Noop>("c")];
    let err = run(teams, 1).err().unwrap();
    assert!(err.to_string().contains("exactly 2 teams"));
}
