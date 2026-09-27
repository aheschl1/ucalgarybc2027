//! Developer CLI: replay inspection, schemas, and SDK generation, no Python. Sees every
//! game in the repo, whatever a build includes.

mod sdk;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ucbc_engine::replay::read_replay;
use ucbc_engine::{Replay, SetReplay, TeamId, TeamInfo};
use ucbc_games::registry;

#[derive(Parser)]
#[command(name = "ucbc-dev", about = "Inspect replays and generate code")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the sets and result of a replay file.
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

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
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
                let dir = games.join(name);
                std::fs::create_dir_all(&dir)?;
                let path = dir.join("_api.py");
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
}
