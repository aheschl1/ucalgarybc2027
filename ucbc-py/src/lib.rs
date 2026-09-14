//! `ucbc._engine`: the match engine as a Python extension module, plus the Python
//! bot runtime.

mod runtime;

use std::path::PathBuf;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;

use runtime::PyTeam;
use ucbc_engine::{EngineError, GameRegistry, MatchConfig, MatchRunner, MatchSpec, TeamSpec};

/// Every game compiled into this wheel.
fn registry() -> GameRegistry {
    // `mut` is unused in a build with no game features.
    #[allow(unused_mut)]
    let mut registry = GameRegistry::new();
    #[cfg(feature = "tictactoe")]
    registry.register::<ucbc_tictactoe::TicTacToe>();
    registry
}

fn to_pyerr(e: EngineError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Runs a match between Python teams and returns the match result as JSON.
#[pyfunction]
#[pyo3(signature = (game, bot_dirs, *, sets = 3, seed = 0, match_id = "local", names = None, replay_path = None, summary_path = None, echo_bot_output = false))]
#[allow(clippy::too_many_arguments)]
fn run_match(
    py: Python<'_>,
    game: &str,
    bot_dirs: Vec<PathBuf>,
    sets: u32,
    seed: u64,
    match_id: &str,
    names: Option<Vec<String>>,
    replay_path: Option<PathBuf>,
    summary_path: Option<PathBuf>,
    echo_bot_output: bool,
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
    let teams = names
        .into_iter()
        .zip(bot_dirs)
        .map(|(name, dir)| match PyTeam::read(&dir, game) {
            Ok(team) => {
                let team = Arc::new(team);
                TeamSpec::new(name, Box::new(move |ctx| team.spawn(ctx)))
            }
            Err(failure) => TeamSpec::unavailable(name, failure),
        })
        .collect();
    let config = MatchConfig::new(game, sets, seed, 0);
    let mut spec = MatchSpec::new(match_id, config, teams).echo_bot_output(echo_bot_output);
    if let Some(path) = replay_path {
        spec = spec.replay_path(path);
    }
    if let Some(path) = summary_path {
        spec = spec.summary_path(path);
    }
    let registry = registry();
    let report = py
        .detach(|| MatchRunner::new(&registry, spec)?.run())
        .map_err(to_pyerr)?;
    serde_json::to_string(&report.replay.result).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pymodule]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_match, m)?)?;
    m.add("GAMES", registry().names())?;
    m.add("__version__", ucbc_engine::ENGINE_VERSION)?;
    Ok(())
}
