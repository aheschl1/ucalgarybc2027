//! Developer CLI: Rust-vs-Rust tic-tac-toe matches and replay inspection, no Python.

mod bots;
mod sdk;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ucbc_engine::replay::read_replay;
use ucbc_engine::{
    BotResourceLimit, Game, GameRegistry, MatchConfig, MatchRunner, MatchSpec, Replay, SetReplay,
    TeamId, TeamInfo,
};
use ucbc_tictactoe::{Board, TicTacToe};

#[derive(Parser)]
#[command(name = "ucbc-dev", about = "Run and inspect matches without Python")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play two built-in Rust bots against each other.
    Run {
        /// Bot kind for team a.
        #[arg(long, default_value = "first-empty", value_parser = bots::KINDS)]
        a: String,
        /// Bot kind for team b.
        #[arg(long, default_value = "random", value_parser = bots::KINDS)]
        b: String,
        #[arg(long, default_value_t = 3)]
        sets: u32,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Time budget per bot step, in milliseconds.
        #[arg(long, default_value_t = 500)]
        step_ms: u64,
        /// Memory budget per bot, in mebibytes.
        #[arg(long, default_value_t = 1024)]
        memory_mb: u64,
        #[arg(long)]
        replay: Option<PathBuf>,
        #[arg(long)]
        summary: Option<PathBuf>,
    },
    /// Print the sets and final boards of a replay file.
    Inspect { replay: PathBuf },
    /// Print what a game exposes to bots, as JSON Schema.
    Api { game: String },
    /// Print the replay format, as JSON Schema.
    ReplaySchema,
    /// Write every game's Python API module under `games`, as `<game>/_api.py`.
    GenSdk {
        /// The `ucbc/games` directory.
        games: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn registry() -> GameRegistry {
    let mut registry = GameRegistry::new();
    registry.register::<TicTacToe>();
    registry
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::Run {
            a,
            b,
            sets,
            seed,
            step_ms,
            memory_mb,
            replay,
            summary,
        } => {
            let teams = vec![
                bots::team(&a, &a).expect("validated by clap"),
                bots::team(&b, &b).expect("validated by clap"),
            ];
            let mut spec = MatchSpec::new(
                "dev",
                MatchConfig::new(
                    TicTacToe::NAME,
                    sets,
                    seed,
                    2,
                    BotResourceLimit::new(step_ms, memory_mb << 20),
                ),
                teams,
            );
            if let Some(path) = replay {
                spec = spec.replay_path(path);
            }
            if let Some(path) = summary {
                spec = spec.summary_path(path);
            }
            let report = MatchRunner::new(&registry(), spec)?.run()?;
            print_replay(&report.replay);
            Ok(())
        }
        Command::Inspect { replay } => {
            let replay = read_replay(&replay)?;
            println!(
                "match {} (engine {})",
                replay.match_id, replay.engine_version
            );
            print_replay(&replay);
            Ok(())
        }
        Command::Api { game } => {
            println!("{}", serde_json::to_string_pretty(&registry().api(&game)?)?);
            Ok(())
        }
        Command::ReplaySchema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schemars::schema_for!(Replay))?
            );
            Ok(())
        }
        Command::GenSdk { games } => {
            let registry = registry();
            for name in registry.names() {
                let module = sdk::generate(&registry.api(name)?)?;
                let path = games.join(name).join("_api.py");
                std::fs::write(&path, module)?;
                println!("wrote {}", path.display());
            }
            Ok(())
        }
    }
}

fn print_replay(replay: &Replay) {
    let name = |team: TeamId| replay.teams[team.0 as usize].name.as_str();
    for set in &replay.sets {
        print_set(set, &replay.teams);
    }
    let scores: Vec<String> = replay
        .result
        .set_wins
        .iter()
        .enumerate()
        .map(|(i, w)| format!("{} {w}", replay.teams[i].name))
        .collect();
    match replay.result.winner_team {
        Some(t) => println!("Result: {} wins ({})", name(t), scores.join(", ")),
        None => println!("Result: tie ({})", scores.join(", ")),
    }
}

fn print_set(set: &SetReplay, teams: &[TeamInfo]) {
    let r = &set.result;
    let verdict = match r.winner_team {
        Some(t) => format!("{} wins by {}", teams[t.0 as usize].name, r.reason),
        None => "draw".to_string(),
    };
    let detail = if r.detail.is_empty() {
        String::new()
    } else {
        format!(" ({})", r.detail)
    };
    println!(
        "Set {}: {verdict} after {} ticks{detail}",
        set.index + 1,
        r.ticks
    );
    if let Some(last) = set.ticks.last()
        && let Ok(board) = serde_json::from_value::<Board>(last.state_after.clone())
    {
        for line in board.render().lines() {
            println!("  {line}");
        }
    }
}
