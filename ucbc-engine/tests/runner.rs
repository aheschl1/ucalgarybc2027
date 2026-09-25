mod support;

use rand::{RngExt as _, SeedableRng};
use serde_json::json;
use support::counting_game::CountingGame;
use support::scripted;
use ucbc_engine::GameStatus;
use ucbc_engine::replay::read_replay;
use ucbc_engine::rng::ChaCha8Rng;
use ucbc_engine::{
    ActionError, BotFailure, BotId, BotResourceLimit, EngineError, GameRegistry, MatchConfig,
    MatchRunner, MatchSpec, Reason, StepResult, TeamId, TeamSpec,
};

fn registry() -> GameRegistry {
    let mut r = GameRegistry::new();
    r.register::<CountingGame>();
    r
}

const LIMITS: BotResourceLimit = BotResourceLimit::new(5, 1 << 20);

fn config(sets: u32, seed: u64) -> MatchConfig {
    MatchConfig::new("counting", sets, seed, 0, LIMITS)
}

fn spec(teams: Vec<TeamSpec>, sets: u32, seed: u64) -> MatchSpec {
    MatchSpec::new("test", config(sets, seed), teams)
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
    let cfg = config(1, 1).game_config(json!({"bots_per_team": 2, "target": 4}));
    let s = MatchSpec::new("test", cfg, vec![make_team("a"), make_team("b")]);
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
    let cfg = config(1, 1).game_config(json!({"bots_per_team": 2, "target": 6}));
    let s = MatchSpec::new("test", cfg, vec![flaky, plus_one("b")]);
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
    let s = MatchSpec::new("test", config(1, 1).max_ticks(7), vec![idle, lazy]);
    let report = MatchRunner::new(&reg, s).unwrap().run().unwrap();
    let set = &report.replay.sets[0];
    assert_eq!(set.result.reason, Reason::Draw);
    assert_eq!(set.result.winner_team, None);
    assert_eq!(set.result.ticks, 7);
    assert!(set.result.detail.contains("tick limit"));
}

#[test]
fn a_bot_id_cannot_be_reused_under_another_team() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    let reg = registry();
    let steps = Arc::new(AtomicU32::new(0));
    let make_team = |name: &str| {
        let steps = steps.clone();
        scripted(name, move |_ctx| {
            steps.fetch_add(1, Ordering::SeqCst);
            StepResult::ok()
        })
    };
    let cfg = config(1, 1)
        .max_ticks(2)
        .game_config(json!({"misattribute_after_first_tick": true}));
    let match_spec = MatchSpec::new("test", cfg, vec![make_team("a"), make_team("b")]);
    let error = MatchRunner::new(&reg, match_spec).unwrap().run().err().unwrap();
    assert!(matches!(error, EngineError::Game(message) if message.contains("bot 0 belongs to team 0, not team 1")));
    // Both bots acted in the first tick; neither acted after the bad ref was scheduled.
    assert_eq!(steps.load(Ordering::SeqCst), 2);
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
    // Everything but the measured step times must repeat.
    let states = |replay: &ucbc_engine::Replay| {
        replay
            .sets
            .iter()
            .flat_map(|s| s.ticks.iter())
            .map(|t| {
                (
                    t.state_after.clone(),
                    t.steps.iter().map(|s| s.actions.clone()).collect(),
                )
            })
            .collect::<Vec<(serde_json::Value, Vec<Vec<serde_json::Value>>)>>()
    };
    let one = run(42);
    let two = run(42);
    let other = run(43);
    assert_eq!(one.result, two.result);
    assert_eq!(states(&one), states(&two));
    assert_ne!(states(&one), states(&other));
}

#[test]
fn bots_spawn_with_the_matchs_limits_and_steps_record_usage() {
    let reg = registry();
    let team = TeamSpec::rust("a", |ctx: &ucbc_engine::SpawnCtx| {
        assert_eq!(ctx.limits, LIMITS);
        support::ScriptedBotWith::new(|ctx| {
            ctx.act(&json!({"type": "increment", "by": 1})).unwrap();
            StepResult::ok().with_memory(4096)
        })
    });
    let report = MatchRunner::new(&reg, spec(vec![team, plus_one("b")], 1, 0))
        .unwrap()
        .run()
        .unwrap();
    let steps = &report.replay.sets[0].ticks[0].steps;
    assert_eq!(steps[0].usage.memory, Some(4096));
    assert_eq!(steps[1].usage.memory, None);
    assert_eq!(report.replay.config.limits, LIMITS);
}

#[test]
fn engine_time_answering_a_bot_is_not_the_bots() {
    let reg = registry();
    let waits = scripted("waits", |ctx| {
        ctx.query(&json!({"type": "slow", "ms": 20})).unwrap();
        assert!(ctx.engine_time() >= std::time::Duration::from_millis(20));
        ctx.act(&json!({"type": "increment", "by": 1})).unwrap();
        StepResult::ok()
    });
    let report = MatchRunner::new(&reg, spec(vec![waits, plus_one("b")], 1, 0))
        .unwrap()
        .run()
        .unwrap();
    let step = &report.replay.sets[0].ticks[0].steps[0];
    assert!(step.usage.time_us < 20_000, "{}", step.usage.time_us);
}

#[test]
fn config_validation() {
    let reg = registry();
    let pair = || vec![plus_one("a"), plus_one("b")];
    let with = |cfg: MatchConfig, teams| MatchRunner::new(&reg, MatchSpec::new("test", cfg, teams));
    assert!(matches!(
        with(config(0, 1), pair()).err(),
        Some(EngineError::Config(_))
    ));
    assert!(matches!(
        with(MatchConfig::new("counting", 1, 1, 3, LIMITS), pair()).err(),
        Some(EngineError::Config(_))
    ));
    assert!(matches!(
        with(config(1, 1).max_ticks(0), pair()).err(),
        Some(EngineError::Config(_))
    ));
    assert!(matches!(
        with(
            MatchConfig::new("chess", 1, 1, 0, LIMITS),
            vec![plus_one("a")]
        )
        .err(),
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
    let s = spec(vec![plus_one("a"), plus_one("b")], 2, 5)
        .replay_path(&replay_path)
        .summary_path(&summary_path);
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

#[test]
fn set_hook_sees_each_set_and_its_error_aborts_the_match() {
    let reg = registry();
    let (tx, rx) = std::sync::mpsc::channel();
    let s = spec(vec![plus_one("a"), plus_one("b")], 3, 1).on_set(Box::new(move |set| {
        tx.send(set.result.index).unwrap();
        Ok(())
    }));
    let report = MatchRunner::new(&reg, s).unwrap().run().unwrap();
    assert_eq!(rx.iter().collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(report.replay.sets.len(), 3);

    let s = spec(vec![plus_one("a"), plus_one("b")], 3, 1).on_set(Box::new(|set| {
        if set.result.index == 1 {
            Err(EngineError::Callback("no more".into()))
        } else {
            Ok(())
        }
    }));
    let Err(err) = MatchRunner::new(&reg, s).unwrap().run() else {
        panic!("a failing hook must abort the match");
    };
    assert!(matches!(err, EngineError::Callback(_)));
    assert_eq!(err.to_string(), "callback failed: no more");
}
