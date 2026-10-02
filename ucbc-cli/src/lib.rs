//! `ucbc._engine`: the match engine as a Python extension module, running Python teams
//! as wasm bots.

mod bot;

use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use bot::PyTeam;
use ucbc_engine::{BotResourceLimit, MatchConfig, MatchRunner, MatchSpec, TeamSpec};
use ucbc_wasm::Runtime;

fn to_pyerr(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(format!("{e:#}"))
}

/// Runs a match between Python teams and returns the match result as JSON.
/// `game_config` is JSON for the game, recorded in the replay.
#[pyfunction]
#[pyo3(signature = (game, bot_dirs, *, step_ms, memory_bytes, sets = 3, seed = 0, match_id = "local", names = None, replay_path = None, summary_path = None, echo_bot_output = false, game_config = None))]
#[allow(clippy::too_many_arguments)]
fn run_match(
    py: Python<'_>,
    game: &str,
    bot_dirs: Vec<PathBuf>,
    step_ms: u64,
    memory_bytes: u64,
    sets: u32,
    seed: u64,
    match_id: &str,
    names: Option<Vec<String>>,
    replay_path: Option<PathBuf>,
    summary_path: Option<PathBuf>,
    echo_bot_output: bool,
    game_config: Option<&str>,
) -> PyResult<String> {
    let names = names.unwrap_or_else(|| {
        bot_dirs
            .iter()
            .map(|d| {
                d.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| d.display().to_string())
            })
            .collect()
    });
    if names.len() != bot_dirs.len() {
        return Err(PyRuntimeError::new_err("names must match bot_dirs"));
    }
    // The installed package holds the runtime under `runtime/`.
    let init: PathBuf = py.import("ucbc")?.getattr("__file__")?.extract()?;
    let package = init.parent().unwrap_or(&init);
    let runtime = Arc::new(Runtime::new(package).map_err(to_pyerr)?);
    let limits = BotResourceLimit::new(step_ms, memory_bytes);
    let teams = names
        .into_iter()
        .zip(bot_dirs)
        .map(
            |(name, dir)| match PyTeam::read(runtime.clone(), &dir, game, &limits) {
                Ok(team) => TeamSpec::new(name, Box::new(move |ctx| team.spawn(ctx))),
                Err(failure) => TeamSpec::unavailable(name, failure),
            },
        )
        .collect();
    let registry = ucbc_games::registry();
    let mut config = MatchConfig::new(game, sets, seed, 0, limits);
    if let Some(json) = game_config {
        config = config.game_config(serde_json::from_str(json).map_err(to_pyerr)?);
    }
    let mut spec = MatchSpec::new(match_id, config, teams).echo_bot_output(echo_bot_output);
    if let Some(path) = replay_path {
        spec = spec.replay_path(path);
    }
    if let Some(path) = summary_path {
        spec = spec.summary_path(path);
    }
    let report = py
        .detach(|| MatchRunner::new(&registry, spec)?.run())
        .map_err(to_pyerr)?;
    serde_json::to_string(&report.replay.result).map_err(to_pyerr)
}

#[pymodule]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_match, m)?)?;
    m.add("GAMES", ucbc_games::registry().names())?;
    m.add("__version__", ucbc_engine::ENGINE_VERSION)?;
    Ok(())
}
