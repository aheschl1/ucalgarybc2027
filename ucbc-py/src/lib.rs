//! `ucbc_engine._engine`: the match engine as a Python extension module, plus the Python
//! bot runtime.

mod process;

use std::path::PathBuf;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::json;

use process::PyTeam;
use ucbc_engine::{
    BotResourceLimit, EngineError, GameRegistry, MatchConfig, MatchRunner, MatchSpec, SetReplay,
    TeamSpec,
};

/// Every game compiled into this wheel.
fn registry() -> GameRegistry {
    // `mut` is unused in a build with no game features.
    #[allow(unused_mut)]
    let mut registry = GameRegistry::new();
    #[cfg(feature = "tictactoe")]
    registry.register::<ucbc_tictactoe::TicTacToe>();
    registry
}

fn to_pyerr(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// A hook's Python exception is re-raised as itself.
fn engine_to_pyerr(e: EngineError) -> PyErr {
    match e {
        EngineError::Callback(inner) => match inner.downcast::<PyErr>() {
            Ok(err) => *err,
            Err(other) => to_pyerr(other),
        },
        other => to_pyerr(other),
    }
}

/// Runs a match between Python teams and returns the match result as JSON. `on_set` is
/// called with each set's replay as JSON when the set ends.
#[pyfunction]
#[pyo3(signature = (game, bot_dirs, *, step_ms, memory_bytes, sets = 3, seed = 0, match_id = "local", names = None, replay_path = None, summary_path = None, echo_bot_output = false, on_set = None))]
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
    on_set: Option<Py<PyAny>>,
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
    let python: PathBuf = py.import("sys")?.getattr("executable")?.extract()?;
    let teams = names
        .into_iter()
        .zip(bot_dirs)
        .map(|(name, dir)| match PyTeam::read(&python, &dir, game) {
            Ok(team) => TeamSpec::new(name, Box::new(move |ctx| team.spawn(ctx))),
            Err(failure) => TeamSpec::unavailable(name, failure),
        })
        .collect();
    let registry = registry();
    let limits = BotResourceLimit::new(step_ms, memory_bytes);
    let config = MatchConfig::new(game, sets, seed, 0, limits);
    let mut spec = MatchSpec::new(match_id, config, teams).echo_bot_output(echo_bot_output);
    if let Some(path) = replay_path {
        spec = spec.replay_path(path);
    }
    if let Some(path) = summary_path {
        spec = spec.summary_path(path);
    }
    if let Some(hook) = on_set {
        spec = spec.on_set(Box::new(move |set: &SetReplay| {
            let json = serde_json::to_string(set)?;
            Python::attach(|py| hook.call1(py, (json,)))
                .map(drop)
                .map_err(|e| EngineError::Callback(Box::new(e)))
        }));
    }
    let report = py
        .detach(|| MatchRunner::new(&registry, spec)?.run())
        .map_err(engine_to_pyerr)?;
    serde_json::to_string(&report.replay.result).map_err(to_pyerr)
}

/// System calls a bot process may not make: signalling, forking, threads, exec,
/// other processes' memory, opening or changing files, and sockets. Everything it
/// needs from disk is imported before the filter goes on.
const REFUSED: &[&str] = &[
    "kill",
    "tkill",
    "tgkill",
    "rt_sigqueueinfo",
    "rt_tgsigqueueinfo",
    "pidfd_send_signal",
    "clone",
    "clone3",
    "execve",
    "execveat",
    "ptrace",
    "process_vm_readv",
    "process_vm_writev",
    "prctl",
    "open",
    "openat",
    "openat2",
    "creat",
    "memfd_create",
    "unlink",
    "unlinkat",
    "rename",
    "renameat",
    "renameat2",
    "mkdir",
    "mkdirat",
    "rmdir",
    "link",
    "linkat",
    "symlink",
    "symlinkat",
    "mknod",
    "mknodat",
    "chmod",
    "fchmodat",
    "chown",
    "lchown",
    "fchownat",
    "truncate",
    "utimensat",
    "sync",
    "syncfs",
    "socket",
    "socketpair",
    #[cfg(target_arch = "x86_64")]
    "fork",
    #[cfg(target_arch = "x86_64")]
    "vfork",
];

/// Locks the calling process down for the rest of its life: it dies with its
/// parent, and the calls in `REFUSED` fail with `EPERM`. `ucbc_engine._bot` calls this
/// after its imports and before running the team's code.
#[pyfunction]
fn lockdown() -> PyResult<()> {
    nix::sys::prctl::set_pdeathsig(nix::sys::signal::Signal::SIGKILL).map_err(to_pyerr)?;
    let arch = seccompiler::TargetArch::try_from(std::env::consts::ARCH).map_err(to_pyerr)?;
    let refused: Vec<_> = REFUSED.iter().map(|s| json!({ "syscall": s })).collect();
    let filter = json!({ "bot": {
        "mismatch_action": "allow",
        "match_action": { "errno": nix::errno::Errno::EPERM as i32 },
        "filter": refused,
    }})
    .to_string();
    let programs = seccompiler::compile_from_json(filter.as_bytes(), arch).map_err(to_pyerr)?;
    seccompiler::apply_filter(&programs["bot"]).map_err(to_pyerr)
}

#[pymodule]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_match, m)?)?;
    m.add_function(wrap_pyfunction!(lockdown, m)?)?;
    m.add("GAMES", registry().names())?;
    m.add("__version__", ucbc_engine::ENGINE_VERSION)?;
    Ok(())
}
