//! The two bots `ucbc-dev` can run. Not reference bots: participants and the
//! reference bots are Python programs under `bots/`.
//!
//! EXAMPLES ONLY
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use serde_json::json;
use ucbc_engine::rng::ChaCha8Rng;
use ucbc_engine::{Bot, SpawnCtx, StepCtx, StepResult, TeamSpec};
use ucbc_tictactoe::BoardView;

pub const KINDS: [&str; 2] = ["first-empty", "random"];

pub fn team(kind: &str, name: &str) -> Option<TeamSpec> {
    let spec = match kind {
        "first-empty" => TeamSpec::rust(name, |_: &SpawnCtx| FirstEmpty),
        "random" => TeamSpec::rust(name, |ctx: &SpawnCtx| Random::new(ctx.seed)),
        _ => return None,
    };
    Some(spec)
}

fn empty_cells(ctx: &StepCtx<'_>) -> Vec<(u32, u32)> {
    let view: BoardView =
        serde_json::from_value(ctx.query(&json!({"type": "board"})).expect("board query"))
            .expect("board shape");
    view.board.empty_cells()
}

fn place(ctx: &mut StepCtx<'_>, (row, col): (u32, u32)) {
    ctx.act(&json!({"type": "place", "row": row, "col": col}))
        .expect("empty cell is free");
}

/// First empty cell, row-major.
struct FirstEmpty;

impl Bot for FirstEmpty {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        if let Some(&cell) = empty_cells(ctx).first() {
            place(ctx, cell);
        }
        StepResult::ok()
    }
}

/// Random empty cell, seeded by the engine.
struct Random {
    rng: ChaCha8Rng,
}

impl Random {
    fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl Bot for Random {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult {
        if let Some(&cell) = empty_cells(ctx).choose(&mut self.rng) {
            place(ctx, cell);
        }
        StepResult::ok()
    }
}
