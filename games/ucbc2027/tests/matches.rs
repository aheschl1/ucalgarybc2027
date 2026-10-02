//! Full matches through the engine, with minimal bots defined here.

use serde_json::json;
use ucbc_2027::{BotType, Item, Ucbc2027};
use ucbc_engine::{
    Bot, BotFailure, BotId, BotResourceLimit, GameRegistry, MatchConfig, MatchRunner, MatchSpec,
    Reason, Replay, SpawnCtx, StepCtx, StepResult, TeamId, TeamSpec,
};

const LIMITS: BotResourceLimit = BotResourceLimit::new(500, 1 << 30);
const TICKS: u32 = 5;

/// Finds its base in its team's corner (team 0 top-left, team 1 bottom-right), then
/// does nothing.
#[derive(Default)]
struct Noop;

impl Bot for Noop {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let team = ctx.team.id.0;
        let corner = 15 * team;
        let item: Option<Item> = serde_json::from_value(
            ctx.query(&json!({"type": "item", "x": corner, "y": corner}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            item,
            Some(Item::Bot {
                team,
                bot_type: BotType::Base
            })
        );
        assert!(
            ctx.query(&json!({"type": "item", "x": 99, "y": 0}))
                .is_err()
        );
        assert_eq!(ctx.act(&json!({"type": "noop"})).unwrap(), json!(null));
        StepResult::ok()
    }
}

/// On its first step a base spawns a dino on the square diagonal to it; any other bot
/// tries to spawn and is refused.
#[derive(Default)]
struct Spawner;

impl Bot for Spawner {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let near = if ctx.team.id.0 == 0 { 1 } else { 14 };
        let spawn = |x: u32, y: u32, bot_type: serde_json::Value| json!({"type": "spawn", "x": x, "y": y, "bot_type": bot_type});
        let dino = json!({"type": "dino", "level": 1});
        let is_base = ctx.bot == BotId(0) || ctx.bot == BotId(1);
        if is_base && ctx.tick == 0 {
            // Not next to the base, a second base, then a taken square.
            assert!(ctx.act(&spawn(5, 5, dino.clone())).is_err());
            assert!(
                ctx.act(&spawn(near, near, json!({"type": "base"})))
                    .is_err()
            );
            ctx.act(&spawn(near, near, dino.clone())).unwrap();
            assert!(ctx.act(&spawn(near, near, dino)).is_err());
        } else if !is_base {
            assert!(ctx.act(&spawn(near + 1, near, dino)).is_err());
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

#[test]
fn sets_run_to_the_tick_limit_and_draw() {
    let replay = run(vec![team::<Noop>("a"), team::<Noop>("b")], 2).unwrap();
    for set in &replay.sets {
        assert_eq!(set.result.reason, Reason::Draw);
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
    assert_eq!(replay.result.winner_team, None);
}

#[test]
fn a_failed_bot_sits_out_the_set() {
    let replay = run(vec![team::<Failing>("a"), team::<Noop>("b")], 1).unwrap();
    assert_eq!(replay.sets[0].result.reason, Reason::Draw);
    let steppers = steppers(&replay, 0);
    assert_eq!(steppers.iter().filter(|&&t| t == 0).count(), 1);
    assert_eq!(steppers.iter().filter(|&&t| t == 1).count(), TICKS as usize);
    assert_eq!(replay.sets[0].ticks[0].steps[0].team, TeamId(0));
    // The failed base leaves the board.
    assert_eq!(
        replay.sets[0].ticks[0].state_after["board"][0][0]["item"],
        json!(null)
    );
}

#[test]
fn a_base_spawns_a_dino_that_steps_from_the_next_tick() {
    let replay = run(vec![team::<Spawner>("a"), team::<Spawner>("b")], 1).unwrap();
    let set = &replay.sets[0];
    assert_eq!(set.ticks[0].steps.len(), 2);
    let order: Vec<u64> = set.ticks[1].steps.iter().map(|s| s.bot.0).collect();
    assert_eq!(order, vec![0, 1, 2, 3]);
    let board = &set.ticks[0].state_after["board"];
    assert_eq!(
        board[1][1]["item"],
        json!({"type": "bot", "team": 0, "bot_type": {"type": "dino", "level": 1}})
    );
    assert_eq!(board[14][14]["item"]["team"], json!(1));
}

#[test]
fn needs_exactly_two_teams() {
    let teams = vec![team::<Noop>("a"), team::<Noop>("b"), team::<Noop>("c")];
    let err = run(teams, 1).err().unwrap();
    assert!(err.to_string().contains("exactly 2 teams"));
}
