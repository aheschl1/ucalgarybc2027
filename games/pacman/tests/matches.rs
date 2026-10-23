//! Full matches through the engine on small maps, with scripted bots defined here.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::json;
use ucbc_engine::{
    ActionError, Bot, BotFailure, BotResourceLimit, EngineError, GameRegistry, MatchConfig,
    MatchRunner, MatchSpec, Reason, Replay, SpawnCtx, StepCtx, StepResult, TeamId, TeamSpec,
};
use ucbc_pacman::{Coord, Pacman, Role, Snapshot, View};

const LIMITS: BotResourceLimit = BotResourceLimit::new(500, 1 << 30);

/// A bot run by a plain function of the step and what it sees.
struct Script(fn(&mut StepCtx<'_>, &View) -> StepResult);

impl Bot for Script {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let view: View =
            serde_json::from_value(ctx.query(&json!({"type": "view"})).unwrap()).unwrap();
        (self.0)(ctx, &view)
    }
}

fn team(name: &str, script: fn(&mut StepCtx<'_>, &View) -> StepResult) -> TeamSpec {
    TeamSpec::rust(name, move |_: &SpawnCtx| Script(script))
}

fn go(ctx: &mut StepCtx<'_>, dir: &str) -> Result<Coord, ActionError> {
    ctx.act(&json!({"type": "move", "dir": dir}))
        .map(|at| serde_json::from_value(at).unwrap())
}

fn idle(_: &mut StepCtx<'_>, _: &View) -> StepResult {
    StepResult::ok()
}

fn run(map: &str, a: TeamSpec, b: TeamSpec, sets: u32, ticks: u32) -> Result<Replay, EngineError> {
    let mut reg = GameRegistry::new();
    reg.register::<Pacman>();
    let config = MatchConfig::new("pacman", sets, 0, 2, LIMITS)
        .max_ticks(ticks)
        .game_config(json!({"maps": [BASE64.encode(map)]}));
    Ok(
        MatchRunner::new(&reg, MatchSpec::new("t", config, vec![a, b]))?
            .run()?
            .replay,
    )
}

fn last(replay: &Replay, set: usize) -> Snapshot {
    let state = &replay.sets[set].ticks.last().unwrap().state_after;
    serde_json::from_value(state.clone()).unwrap()
}

#[test]
fn the_first_team_plays_pacman_and_wins_by_clearing_the_maze() {
    fn eat_right(ctx: &mut StepCtx<'_>, view: &View) -> StepResult {
        if view.me.role == Role::Pacman {
            go(ctx, "right").unwrap();
        }
        StepResult::ok()
    }
    let replay = run("#P.. G#", team("a", eat_right), team("b", eat_right), 2, 10).unwrap();
    for (set, pacman) in [(0, 0), (1, 1)] {
        let result = &replay.sets[set].result;
        assert_eq!(result.winner_team, Some(TeamId(pacman)));
        assert_eq!(result.detail, "Pac-Man cleared the maze");
        assert_eq!(result.ticks, 2);
        let snapshot = last(&replay, set);
        assert_eq!(snapshot.pacman_team, pacman);
        assert_eq!(snapshot.maze, vec!["#    G#"]);
        assert_eq!(snapshot.pellets_left, 0);
    }
    assert_eq!(replay.result.set_wins, vec![1, 1]);
}

#[test]
fn a_ghost_catches_pacman() {
    fn chase(ctx: &mut StepCtx<'_>, view: &View) -> StepResult {
        if view.me.role == Role::Ghost {
            go(ctx, "left").unwrap();
        }
        StepResult::ok()
    }
    let replay = run("#P. G#", team("a", chase), team("b", chase), 1, 10).unwrap();
    let result = &replay.sets[0].result;
    assert_eq!(result.winner_team, Some(TeamId(1)));
    assert_eq!(result.detail, "the ghosts caught Pac-Man");
    assert_eq!(result.ticks, 3);
    // Ghosts don't eat.
    assert_eq!(last(&replay, 0).pellets_left, 1);
}

#[test]
fn pacman_is_caught_walking_into_a_ghost() {
    fn walk(ctx: &mut StepCtx<'_>, view: &View) -> StepResult {
        match view.me.role {
            Role::Pacman => drop(go(ctx, "right").unwrap()),
            Role::Ghost if ctx.tick == 0 => drop(go(ctx, "left").unwrap()),
            Role::Ghost => {}
        }
        StepResult::ok()
    }
    let replay = run("#P.  .G#", team("a", walk), team("b", walk), 1, 10).unwrap();
    // Pac-Man eats the last pellet on the ghost's tile, but is caught there first.
    let result = &replay.sets[0].result;
    assert_eq!(result.winner_team, Some(TeamId(1)));
    assert_eq!(result.detail, "the ghosts caught Pac-Man");
    assert_eq!(result.ticks, 4);
}

#[test]
fn refused_moves_change_nothing() {
    fn bump(ctx: &mut StepCtx<'_>, view: &View) -> StepResult {
        if view.me.role != Role::Pacman {
            return StepResult::ok();
        }
        let refused = |r: Result<Coord, ActionError>, why: &str| match r {
            Err(ActionError::Invalid(msg)) => assert!(msg.contains(why), "{msg}"),
            other => panic!("expected a refusal, got {other:?}"),
        };
        match ctx.tick {
            0 => {
                refused(go(ctx, "left"), "wall");
                assert_eq!(go(ctx, "right").unwrap(), Coord::new(2, 0));
                refused(go(ctx, "down"), "already moved");
            }
            1 => refused(go(ctx, "right"), "ghost house"),
            _ => assert!(
                ctx.act(&json!({"type": "move", "dir": "sideways"}))
                    .is_err()
            ),
        }
        StepResult::ok()
    }
    let replay = run("#P.G#\n#...#", team("a", bump), team("b", idle), 1, 3).unwrap();
    let result = &replay.sets[0].result;
    assert_eq!(result.winner_team, Some(TeamId(1)));
    assert_eq!(result.detail, "Pac-Man ran out of time");
    let pacman = last(&replay, 0).agents[0];
    assert_eq!(pacman.pos, Coord::new(2, 0));
    // Only the accepted move is recorded.
    assert_eq!(replay.sets[0].ticks[0].steps[0].actions.len(), 1);
}

#[test]
fn a_power_pellet_lets_pacman_eat_a_slowed_ghost() {
    fn play(ctx: &mut StepCtx<'_>, view: &View) -> StepResult {
        match view.me.role {
            Role::Pacman if ctx.tick == 0 => {
                go(ctx, "right").unwrap();
            }
            Role::Pacman => {}
            Role::Ghost if view.me.frightened > 0 => {
                let moved = go(ctx, "left");
                assert_eq!(
                    moved.is_ok(),
                    ctx.tick.is_multiple_of(2),
                    "tick {}",
                    ctx.tick
                );
            }
            // Eaten and back home: stay there.
            Role::Ghost => {}
        }
        StepResult::ok()
    }
    let replay = run("#Po..G#", team("a", play), team("b", play), 1, 8).unwrap();
    let ticks = &replay.sets[0].ticks;
    let state =
        |t: usize| -> Snapshot { serde_json::from_value(ticks[t].state_after.clone()).unwrap() };
    // Tick 0: Pac-Man eats the power pellet; the ghost, frightened, moves anyway.
    assert_eq!(state(0).agents[1].pos, Coord::new(4, 0));
    assert_eq!(
        state(0).agents[1].frightened,
        ucbc_pacman::rules::FRIGHTENED_TICKS - 1
    );
    // Tick 4: the ghost walks into Pac-Man and is sent home.
    assert_eq!(state(3).ghosts_eaten, 0);
    assert_eq!(state(4).ghosts_eaten, 1);
    assert_eq!(state(4).agents[1].pos, Coord::new(5, 0));
    assert_eq!(state(4).agents[1].frightened, 0);
    assert_eq!(replay.sets[0].result.detail, "Pac-Man ran out of time");
}

#[test]
fn a_failing_pacman_forfeits_and_a_failing_ghost_is_removed() {
    fn fail(_: &mut StepCtx<'_>, _: &View) -> StepResult {
        StepResult::failed(BotFailure::exception("RuntimeError", "I give up"))
    }
    fn eat_right(ctx: &mut StepCtx<'_>, view: &View) -> StepResult {
        if view.me.role == Role::Pacman {
            go(ctx, "right").unwrap();
        }
        StepResult::ok()
    }
    let replay = run("#P...G#", team("a", eat_right), team("b", fail), 2, 10).unwrap();
    // Set 0: b's ghost fails and leaves; Pac-Man clears the maze.
    assert_eq!(replay.sets[0].result.winner_team, Some(TeamId(0)));
    assert_eq!(last(&replay, 0).agents.len(), 1);
    // Set 1: b's Pac-Man fails on its first step.
    let result = &replay.sets[1].result;
    assert_eq!(result.reason, Reason::Forfeit);
    assert_eq!(result.winner_team, Some(TeamId(0)));
    assert!(
        result.detail.contains("RuntimeError: I give up"),
        "{}",
        result.detail
    );
}

#[test]
fn the_view_shows_everyone() {
    fn check(_: &mut StepCtx<'_>, view: &View) -> StepResult {
        assert_eq!(view.maze, vec![" ..GG#"]);
        assert_eq!(view.pellets_left, 2);
        assert_eq!(view.pacman.pos, Coord::new(0, 0));
        let ghosts: Vec<_> = view.ghosts.iter().map(|g| (g.role, g.pos)).collect();
        assert_eq!(
            ghosts,
            vec![
                (Role::Ghost, Coord::new(3, 0)),
                (Role::Ghost, Coord::new(4, 0))
            ]
        );
        assert!(view.me == view.pacman || view.ghosts.contains(&view.me));
        StepResult::ok()
    }
    run("P..GG#", team("a", check), team("b", check), 1, 1).unwrap();
}

#[test]
fn needs_two_teams_and_a_valid_map() {
    let mut reg = GameRegistry::new();
    reg.register::<Pacman>();
    let three = MatchConfig::new("pacman", 1, 0, 3, LIMITS);
    let teams = vec![team("a", idle), team("b", idle), team("c", idle)];
    let err = MatchRunner::new(&reg, MatchSpec::new("t", three, teams))
        .unwrap()
        .run()
        .err()
        .unwrap();
    assert!(err.to_string().contains("exactly 2 teams"), "{err}");

    let err = run("P..", team("a", idle), team("b", idle), 1, 1)
        .err()
        .unwrap();
    assert!(err.to_string().contains("at least one G"), "{err}");
}
