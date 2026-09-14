mod support;

use rand::{RngExt as _, SeedableRng};
use serde_json::json;
use support::counting_game::CountingFactory;
use support::scripted;
use ucbc_engine::GameStatus;
use ucbc_engine::replay::read_replay;
use ucbc_engine::rng::ChaCha8Rng;
use ucbc_engine::{
    ActionError, BotFailure, BotId, EngineError, GameRegistry, MatchConfig, MatchRunner, MatchSpec,
    Reason, StepResult, TeamId, TeamSpec,
};

fn registry() -> GameRegistry {
    let mut r = GameRegistry::new();
    r.register(Box::new(CountingFactory));
    r
}

fn spec(teams: Vec<TeamSpec>, sets: u32, seed: u64) -> MatchSpec {
    MatchSpec::new("test", MatchConfig::new("counting", sets, seed, 0), teams)
}

/// Every bot of the team increments by 1.
fn plus_one(name: &str) -> TeamSpec {
    scripted(name, |ctx| {
        ctx.act(&json!({"type": "increment", "by": 1})).unwrap();
        StepResult::ok()
    })
}

#[test]
fn first_team_alternates_and_scores_add_up() {
    let reg = registry();
    let report = MatchRunner::new(&reg, spec(vec![plus_one("a"), plus_one("b")], 3, 1))
        .unwrap()
        .run()
        .unwrap();
    let r = &report.replay;
    assert_eq!(r.sets.len(), 3);
    assert_eq!(r.sets[0].first_team, TeamId(0));
    assert_eq!(r.sets[1].first_team, TeamId(1));
    assert_eq!(r.sets[2].first_team, TeamId(0));
    // With +1 per bot per tick the first mover reaches 5 first, on tick 5.
    assert_eq!(r.sets[0].result.winner_team, Some(TeamId(0)));
    assert_eq!(r.sets[1].result.winner_team, Some(TeamId(1)));
    assert_eq!(r.sets[0].result.reason, Reason::Win);
    assert_eq!(r.sets[0].result.ticks, 5);
    // The winning tick stops before the second team steps.
    assert_eq!(r.sets[0].ticks[4].steps.len(), 1);
    assert_eq!(r.result.set_wins, vec![2, 1]);
    assert_eq!(r.result.winner_team, Some(TeamId(0)));
    assert_eq!(r.teams[1].name, "b");
    assert_eq!(report.summary.winner_team, Some(TeamId(0)));
    assert_eq!(report.summary.game, "counting");
}

#[test]
fn teams_own_several_bots_and_the_game_interleaves_them() {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    let reg = registry();
    let seen: &'static Mutex<Vec<(u32, u64, u32)>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let spawns: &'static AtomicU32 = Box::leak(Box::new(AtomicU32::new(0)));

    let make_team = |name: &str| {
        TeamSpec::rust(name, move |_spawn| {
            spawns.fetch_add(1, Ordering::SeqCst);
            support::ScriptedBotWith::new(move |ctx| {
                seen.lock()
                    .unwrap()
                    .push((ctx.tick, ctx.bot.id.0, ctx.bot.team.0));
                ctx.act(&json!({"type": "increment", "by": 1})).unwrap();
                StepResult::ok()
            })
        })
    };
    let mut s = spec(vec![make_team("a"), make_team("b")], 1, 1);
    s.config.game_config = Some(json!({"bots_per_team": 2, "target": 4}));
    let report = MatchRunner::new(&reg, s).unwrap().run().unwrap();
    let set = &report.replay.sets[0];

    // Tick 0 order: a's bot 0, b's bot 1, a's bot 2, b's bot 3 (game decides).
    let order: Vec<(u64, u32)> = set.ticks[0]
        .steps
        .iter()
        .map(|s| (s.bot.0, s.team.0))
        .collect();
    assert_eq!(order, vec![(0, 0), (1, 1), (2, 0), (3, 1)]);
    // Two bots per team, +1 each, target 4: team 0 wins on tick 1 with its second bot.
    assert_eq!(set.result.winner_team, Some(TeamId(0)));
    assert_eq!(set.result.ticks, 2);
    assert_eq!(set.ticks[1].steps.len(), 3);
    // One runtime per bot, created lazily; each bot saw its own id.
    assert_eq!(spawns.load(Ordering::SeqCst), 4);
    let seen = seen.lock().unwrap();
    assert!(seen.contains(&(0, 2, 0)));
    assert!(seen.contains(&(1, 3, 1)) || seen.contains(&(0, 3, 1)));
}

#[test]
fn queries_see_state_and_identity() {
    let reg = registry();
    let asker = scripted("asker", |ctx| {
        let v = ctx.query(&json!({"type": "counts"})).unwrap();
        assert_eq!(v["you"], json!(ctx.bot.team.0));
        assert_eq!(v["bot"], json!(ctx.bot.id.0));
        assert_eq!(ctx.team.name, "asker");
        assert!(matches!(
            ctx.query(&json!({"type": "nope"})),
            Err(ucbc_engine::QueryError::UnknownType(_))
        ));
        let response = ctx.act(&json!({"type": "increment", "by": 3})).unwrap();
        assert_eq!(response, json!({ "count": 3 * (ctx.tick + 1) }));
        StepResult::ok().with_stdout("hello")
    });
    let report = MatchRunner::new(&reg, spec(vec![asker, plus_one("b")], 1, 1))
        .unwrap()
        .run()
        .unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(set.result.winner_team, Some(TeamId(0)));
    assert_eq!(set.ticks[0].steps[0].stdout, "hello");
    assert_eq!(set.ticks[0].steps[1].stdout, "");
    assert_eq!(
        set.ticks[0].steps[0].actions,
        vec![json!({"type": "increment", "by": 3})]
    );
}

#[test]
fn rejected_action_is_an_error_the_bot_can_recover_from() {
    let reg = registry();
    let retrier = scripted("retrier", |ctx| {
        let err = ctx
            .act(&json!({"type": "increment", "by": 10}))
            .unwrap_err();
        assert!(matches!(err, ActionError::Invalid(_)));
        assert_eq!(
            ctx.act(&json!({"type": "teleport"})),
            Err(ActionError::UnknownType("teleport".into()))
        );
        assert!(matches!(
            ctx.act(&json!({"type": "increment", "by": "x"})),
            Err(ActionError::Malformed(_))
        ));
        // Nothing was forfeited; a legal action still goes through.
        ctx.act(&json!({"type": "increment", "by": 1})).unwrap();
        let again = ctx.act(&json!({"type": "increment", "by": 1}));
        if matches!(ctx.status(), GameStatus::InProgress) {
            assert_eq!(
                again,
                Err(ActionError::Invalid("already acted this step".into()))
            );
        } else {
            assert_eq!(again, Err(ActionError::SetOver));
        }
        StepResult::ok()
    });
    let report = MatchRunner::new(&reg, spec(vec![retrier, plus_one("b")], 1, 1))
        .unwrap()
        .run()
        .unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(set.result.reason, Reason::Win);
    assert_eq!(set.result.winner_team, Some(TeamId(0)));
    // Only the accepted action is in the replay, and no failure.
    assert_eq!(
        set.ticks[0].steps[0].actions,
        vec![json!({"type": "increment", "by": 1})]
    );
    assert!(set.ticks[0].steps[0].failure.is_none());
}

#[test]
fn a_bot_that_never_acts_is_the_games_business() {
    // The counting game tolerates idle bots; the other team simply wins.
    let reg = registry();
    let idle = scripted("idle", |_ctx| StepResult::ok());
    let report = MatchRunner::new(&reg, spec(vec![idle, plus_one("b")], 1, 1))
        .unwrap()
        .run()
        .unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(set.result.reason, Reason::Win);
    assert_eq!(set.result.winner_team, Some(TeamId(1)));
    assert!(set.ticks[0].steps[0].actions.is_empty());
    assert!(set.ticks[0].steps[0].failure.is_none());
}

#[test]
fn exception_is_reported_to_the_game_and_recorded() {
    let reg = registry();
    let raiser = scripted("raiser", |_ctx| {
        StepResult::failed(
            BotFailure::exception("ValueError", "boom").with_traceback("Traceback ..."),
        )
    });
    let report = MatchRunner::new(&reg, spec(vec![raiser, plus_one("b")], 1, 1))
        .unwrap()
        .run()
        .unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(set.result.reason, Reason::Forfeit);
    assert_eq!(set.result.winner_team, Some(TeamId(1)));
    assert_eq!(
        set.result.detail,
        "team 0 has no bots left: ValueError: boom"
    );
    let f = set.ticks[0].steps[0].failure.as_ref().unwrap();
    assert_eq!(f.kind, "ValueError");
    assert_eq!(f.traceback, "Traceback ...");
    assert_eq!(set.result.ticks, 1);
}

#[test]
fn a_failed_bot_is_removed_but_its_team_plays_on() {
    let reg = registry();
    // Bot id 0 of team 0 crashes on its first step; bot id 2 keeps going.
    let flaky = scripted("flaky", |ctx| {
        if ctx.bot.id == BotId(0) {
            StepResult::failed(BotFailure::Crash("segfault".into()))
        } else {
            ctx.act(&json!({"type": "increment", "by": 2})).unwrap();
            StepResult::ok()
        }
    });
    let mut s = spec(vec![flaky, plus_one("b")], 1, 1);
    s.config.game_config = Some(json!({"bots_per_team": 2, "target": 6}));
    let report = MatchRunner::new(&reg, s).unwrap().run().unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(
        set.ticks[0].steps[0].failure.as_ref().unwrap().kind,
        "Crash"
    );
    // From tick 1 on, only three bots step.
    assert_eq!(set.ticks[1].steps.len(), 3);
    assert!(set.ticks[1].steps.iter().all(|st| st.bot != BotId(0)));
    // Team 0 gains 2 per tick with one bot, team 1 gains 2 with two bots: team 1 moves
    // after team 0's survivor within a tick, so team 0 reaches 6 first on tick 2.
    assert_eq!(set.result.reason, Reason::Win);
    assert_eq!(set.result.winner_team, Some(TeamId(0)));
}

#[test]
fn unavailable_team_forfeits_every_set() {
    let reg = registry();
    let broken = TeamSpec::unavailable("broken", BotFailure::exception("SyntaxError", "line 1"));
    let report = MatchRunner::new(&reg, spec(vec![plus_one("a"), broken], 3, 1))
        .unwrap()
        .run()
        .unwrap();
    for set in &report.replay.sets {
        assert_eq!(set.result.reason, Reason::Forfeit);
        assert_eq!(set.result.winner_team, Some(TeamId(0)));
        assert!(set.result.detail.contains("SyntaxError: line 1"));
        assert_eq!(set.result.ticks, 1);
    }
    assert_eq!(report.replay.result.set_wins, vec![3, 0]);
}

#[test]
fn tick_limit_ends_the_set_as_a_draw() {
    let reg = registry();
    let idle = scripted("idle", |_ctx| StepResult::ok());
    let lazy = scripted("lazy", |_ctx| StepResult::ok());
    let mut s = spec(vec![idle, lazy], 1, 1);
    s.config.max_ticks = 7;
    let report = MatchRunner::new(&reg, s).unwrap().run().unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(set.result.reason, Reason::Draw);
    assert_eq!(set.result.winner_team, None);
    assert_eq!(set.result.ticks, 7);
    assert!(set.result.detail.contains("tick limit"));
}

#[test]
fn seeded_bots_replay_identically() {
    fn random_team(name: &str) -> TeamSpec {
        scripted(name, |ctx| {
            let mut rng = ChaCha8Rng::seed_from_u64(ctx.seed ^ u64::from(ctx.tick));
            let by: u32 = rng.random_range(1..=3);
            ctx.act(&json!({"type": "increment", "by": by})).unwrap();
            StepResult::ok()
        })
    }
    let reg = registry();
    let run = |seed| {
        MatchRunner::new(
            &reg,
            spec(vec![random_team("a"), random_team("b")], 3, seed),
        )
        .unwrap()
        .run()
        .unwrap()
        .replay
    };
    let one = run(42);
    let two = run(42);
    let other = run(43);
    assert_eq!(
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&two).unwrap()
    );
    assert_ne!(
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&other).unwrap()
    );
}

#[test]
fn config_validation() {
    let reg = registry();
    let mut s = spec(vec![plus_one("a"), plus_one("b")], 0, 1);
    assert!(matches!(
        MatchRunner::new(&reg, s).err(),
        Some(EngineError::Config(_))
    ));
    s = spec(vec![plus_one("a"), plus_one("b")], 1, 1);
    s.config.teams = 3;
    assert!(matches!(
        MatchRunner::new(&reg, s).err(),
        Some(EngineError::Config(_))
    ));
    s = spec(vec![plus_one("a"), plus_one("b")], 1, 1);
    s.config.max_ticks = 0;
    assert!(matches!(
        MatchRunner::new(&reg, s).err(),
        Some(EngineError::Config(_))
    ));
    s = spec(vec![plus_one("a")], 1, 1);
    s.config.game = "chess".into();
    assert!(matches!(
        MatchRunner::new(&reg, s).err(),
        Some(EngineError::UnknownGame(_))
    ));
}

#[test]
fn replay_and_summary_files_round_trip() {
    let reg = registry();
    let dir = std::env::temp_dir().join(format!("ucbc-engine-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let replay_path = dir.join("replay.json");
    let summary_path = dir.join("summary.json");
    let mut s = spec(vec![plus_one("a"), plus_one("b")], 2, 5);
    s.replay_path = Some(replay_path.clone());
    s.summary_path = Some(summary_path.clone());
    let report = MatchRunner::new(&reg, s).unwrap().run().unwrap();

    let read_back = read_replay(&replay_path).unwrap();
    assert_eq!(read_back, report.replay);
    let text = std::fs::read_to_string(&summary_path).unwrap();
    let summary: ucbc_engine::Summary = serde_json::from_str(&text).unwrap();
    assert_eq!(summary.set_wins, vec![1, 1]);
    assert_eq!(summary.winner_team, None);
    assert_eq!(
        summary.replay_path.as_deref(),
        Some(replay_path.to_str().unwrap())
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
