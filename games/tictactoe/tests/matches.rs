//! Full matches through the engine, with minimal bots defined here.

use serde_json::json;
use ucbc_engine::{
    ActionError, Bot, BotFailure, BotResourceLimit, GameRegistry, MatchConfig, MatchRunner,
    MatchSpec, Reason, SpawnCtx, StepCtx, StepResult, TeamId, TeamSpec,
};
use ucbc_tictactoe::{BoardView, Cell, TicTacToe};

fn board(ctx: &mut StepCtx<'_>) -> BoardView {
    serde_json::from_value(ctx.query(&json!({"type": "board"})).unwrap()).unwrap()
}

fn empty_cells(b: &BoardView) -> Vec<(u32, u32)> {
    b.board.empty_cells()
}

fn place(ctx: &mut StepCtx<'_>, row: u32, col: u32) -> Result<(), ActionError> {
    ctx.act(&json!({"type": "place", "row": row, "col": col}))
        .map(|_| ())
}

struct FirstEmpty;

impl Bot for FirstEmpty {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        if let Some(&(r, c)) = empty_cells(&board(ctx)).first() {
            place(ctx, r, c).unwrap();
        }
        StepResult::ok()
    }
}

struct Failing;

impl Bot for Failing {
    fn step(&mut self, _ctx: &mut StepCtx<'_>) -> StepResult {
        StepResult::failed(BotFailure::exception("RuntimeError", "I give up"))
    }
}

struct Idle;

impl Bot for Idle {
    fn step(&mut self, _ctx: &mut StepCtx<'_>) -> StepResult {
        StepResult::ok()
    }
}

fn team<B: Bot + Default + 'static>(name: &str) -> TeamSpec {
    TeamSpec::rust(name, |_: &SpawnCtx| B::default())
}

impl Default for FirstEmpty {
    fn default() -> Self {
        FirstEmpty
    }
}
impl Default for Failing {
    fn default() -> Self {
        Failing
    }
}
impl Default for Idle {
    fn default() -> Self {
        Idle
    }
}

const LIMITS: BotResourceLimit = BotResourceLimit::new(500, 1 << 30);

fn run(a: TeamSpec, b: TeamSpec, sets: u32, seed: u64) -> ucbc_engine::Replay {
    let mut reg = GameRegistry::new();
    reg.register::<TicTacToe>();
    let spec = MatchSpec::new(
        "t",
        MatchConfig::new("tictactoe", sets, seed, 2, LIMITS),
        vec![a, b],
    );
    MatchRunner::new(&reg, spec).unwrap().run().unwrap().replay
}

fn final_cells(set: &ucbc_engine::SetReplay) -> Vec<Cell> {
    serde_json::from_value(set.ticks.last().unwrap().state_after["cells"].clone()).unwrap()
}

#[test]
fn first_empty_mirror_match_is_won_by_x_on_the_diagonal() {
    let replay = run(team::<FirstEmpty>("a"), team::<FirstEmpty>("b"), 3, 0);
    let s0 = &replay.sets[0];
    assert_eq!(s0.result.reason, Reason::Win);
    assert_eq!(s0.result.winner_team, Some(TeamId(0)));
    // Seven moves: three full ticks of two steps, then X's winning step ends tick 3.
    assert_eq!(s0.result.ticks, 4);
    use Cell::{Empty as E, O, X};
    assert_eq!(final_cells(s0), vec![X, O, X, O, X, O, X, E, E]);
    let steppers: Vec<u32> = s0
        .ticks
        .iter()
        .flat_map(|t| t.steps.iter().map(|s| s.team.0))
        .collect();
    assert_eq!(steppers, vec![0, 1, 0, 1, 0, 1, 0]);
    assert_eq!(s0.ticks[3].steps.len(), 1);
    // Team 1 plays X in set 1 and wins the same way.
    assert_eq!(replay.sets[1].result.winner_team, Some(TeamId(1)));
    assert_eq!(replay.result.set_wins, vec![2, 1]);
    assert_eq!(replay.result.winner_team, Some(TeamId(0)));
}

#[test]
fn failing_bot_forfeits_to_the_opponent() {
    let replay = run(team::<FirstEmpty>("a"), team::<Failing>("b"), 2, 1);
    for set in &replay.sets {
        assert_eq!(set.result.reason, Reason::Forfeit);
        assert_eq!(set.result.winner_team, Some(TeamId(0)));
        assert!(set.result.detail.contains("RuntimeError: I give up"));
    }
    // Set 0: a moves, then b fails in the same tick. Set 1: b moves first and fails.
    let s0 = &replay.sets[0].ticks[0].steps;
    assert_eq!(s0.len(), 2);
    assert_eq!(s0[1].failure.as_ref().unwrap().kind, "RuntimeError");
    assert_eq!(replay.sets[1].result.ticks, 1);
    assert_eq!(replay.sets[1].ticks[0].steps.len(), 1);
}

/// First-empty play, but raises right after the move that wins set 0 (tick 3).
struct WinThenRaise;

impl Bot for WinThenRaise {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let (r, c) = empty_cells(&board(ctx))[0];
        place(ctx, r, c).unwrap();
        if ctx.set_index == 0 && ctx.tick == 3 {
            return StepResult::failed(BotFailure::exception("ValueError", "after winning"));
        }
        StepResult::ok()
    }
}

#[test]
fn an_exception_after_the_winning_move_does_not_undo_the_win() {
    let team_a = TeamSpec::rust("a", |_: &SpawnCtx| WinThenRaise);
    let replay = run(team_a, team::<FirstEmpty>("b"), 1, 1);
    let set = &replay.sets[0];
    assert_eq!(set.result.reason, Reason::Win);
    assert_eq!(set.result.winner_team, Some(TeamId(0)));
    let last = set.ticks.last().unwrap().steps.last().unwrap();
    assert_eq!(last.actions.len(), 1);
    assert_eq!(last.failure.as_ref().unwrap().kind, "ValueError");
}

#[test]
fn placing_nothing_forfeits() {
    let replay = run(team::<Idle>("a"), team::<FirstEmpty>("b"), 1, 1);
    let set = &replay.sets[0];
    assert_eq!(set.result.reason, Reason::Forfeit);
    assert_eq!(set.result.winner_team, Some(TeamId(1)));
    assert!(set.result.detail.contains("did not place"));
    assert_eq!(set.result.ticks, 1);
}

/// Insists on (0, 0), then falls back to the first empty cell.
struct Stubborn;

impl Bot for Stubborn {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        let b = board(ctx);
        assert_eq!(b.to_move, b.you);
        match place(ctx, 0, 0) {
            Ok(()) => {}
            Err(ActionError::Invalid(msg)) => {
                assert!(msg.contains("occupied"), "{msg}");
                assert!(matches!(place(ctx, 9, 9), Err(ActionError::Invalid(_))));
                let (r, c) = empty_cells(&b)[0];
                place(ctx, r, c).unwrap();
            }
            Err(e) => panic!("unexpected {e:?}"),
        }
        assert!(matches!(
            place(ctx, 2, 2),
            Err(ActionError::Invalid(_) | ActionError::SetOver)
        ));
        StepResult::ok()
    }
}

#[test]
fn rejected_moves_are_recoverable_and_only_accepted_ones_are_recorded() {
    let stubborn = TeamSpec::rust("stubborn", |_: &SpawnCtx| Stubborn);
    let replay = run(stubborn, team::<FirstEmpty>("b"), 1, 1);
    let set = &replay.sets[0];
    assert_eq!(set.result.reason, Reason::Win);
    for tick in &set.ticks {
        assert_eq!(tick.steps[0].actions.len(), 1);
        assert!(tick.steps[0].failure.is_none());
    }
    assert_eq!(
        set.ticks[0].steps[0].actions[0],
        json!({"type": "place", "row": 0, "col": 0})
    );
}

#[test]
fn board_query_reports_marks_and_turn() {
    struct Checker;
    impl Bot for Checker {
        fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
            let b = board(ctx);
            // Team 0 plays X and steps first in set 0, O and second in set 1.
            let (expected_you, offset) = if ctx.set_index == 0 {
                (Cell::X, 0)
            } else {
                (Cell::O, 1)
            };
            assert_eq!(b.you, expected_you);
            assert_eq!(b.to_move, expected_you);
            assert_eq!(b.turn, 2 * ctx.tick + offset);
            assert_eq!(empty_cells(&b).len(), 9 - b.turn as usize);
            let (r, c) = empty_cells(&b)[0];
            place(ctx, r, c).unwrap();
            StepResult::ok()
        }
    }
    let checker = TeamSpec::rust("checker", |_: &SpawnCtx| Checker);
    run(checker, team::<FirstEmpty>("b"), 2, 1);
}

#[test]
fn needs_exactly_two_teams() {
    let mut reg = GameRegistry::new();
    reg.register::<TicTacToe>();
    let spec = MatchSpec::new(
        "t",
        MatchConfig::new("tictactoe", 1, 0, 3, LIMITS),
        vec![team::<Idle>("a"), team::<Idle>("b"), team::<Idle>("c")],
    );
    let err = MatchRunner::new(&reg, spec).unwrap().run().err().unwrap();
    assert!(err.to_string().contains("exactly 2 teams"));
}
