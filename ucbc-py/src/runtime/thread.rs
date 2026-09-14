//! The bot thread: runs `main.py` once into a fresh namespace, then one step per
//! `Step` message.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use super::api::Api;
use super::stdout::CapturedStream;
use super::{FromBot, Link, PyTeam, ToBot};
use ucbc_engine::{BotFailure, BotRef, SpawnCtx, StepResult};

/// Everything the thread needs, copied out of the spawn context.
pub(super) struct BotSpawn {
    bot: BotRef,
    team_name: String,
    seed: u64,
    team: Arc<PyTeam>,
}

impl BotSpawn {
    pub(super) fn new(ctx: &SpawnCtx, team: Arc<PyTeam>) -> Self {
        Self {
            bot: ctx.bot,
            team_name: ctx.team.name.clone(),
            seed: ctx.seed,
            team,
        }
    }
}

struct Loaded {
    step_fn: Py<PyAny>,
    make_game: Py<PyAny>,
    memory: Py<PyDict>,
    stream: Py<CapturedStream>,
}

pub(super) fn run(spawn: BotSpawn, link: Arc<Link>) {
    let loaded = Python::attach(|py| load(py, &spawn).map_err(|e| failure_from(py, &e)));
    let outcome = loaded.as_ref().map(|_| ()).map_err(Clone::clone);
    if link.to_runner.send(FromBot::Loaded(outcome)).is_err() {
        return;
    }
    let Ok(loaded) = loaded else {
        return;
    };

    while let Some(ToBot::Step { set_index, tick }) = link.recv() {
        let result = Python::attach(|py| run_step(py, &spawn, &loaded, &link, set_index, tick));
        if link.to_runner.send(FromBot::Done(result)).is_err() {
            break;
        }
    }
    Python::attach(|_py| drop(loaded));
}

/// Executes the team's code into a fresh namespace. Output written while loading
/// stays in the stream and is reported with the first step.
fn load(py: Python<'_>, spawn: &BotSpawn) -> PyResult<Loaded> {
    let stream = Py::new(py, CapturedStream::default())?;
    let make_game = PyModule::import(py, "ucbc._runtime")?.getattr("make_game")?;
    let step_fn = with_captured_output(py, &stream, || {
        let builtins = PyModule::import(py, "builtins")?;
        let globals = PyDict::new(py);
        globals.set_item("__name__", format!("ucbc_bot_{}", spawn.bot.id))?;
        globals.set_item("__file__", spawn.team.path.to_string_lossy())?;
        globals.set_item("__builtins__", &builtins)?;
        builtins
            .getattr("exec")?
            .call1((spawn.team.code.bind(py), &globals))?;
        globals.get_item("step")?.ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err("main.py must define step(game)")
        })
    })?;
    Ok(Loaded {
        step_fn: step_fn.unbind(),
        make_game: make_game.unbind(),
        memory: PyDict::new(py).unbind(),
        stream,
    })
}

fn run_step(
    py: Python<'_>,
    spawn: &BotSpawn,
    loaded: &Loaded,
    link: &Arc<Link>,
    set_index: u32,
    tick: u32,
) -> StepResult {
    let raw = match Py::new(
        py,
        Api::new(
            link.clone(),
            spawn.bot,
            spawn.team_name.clone(),
            set_index,
            tick,
            spawn.seed,
            spawn.team.game.clone(),
            loaded.memory.clone_ref(py),
        ),
    ) {
        Ok(raw) => raw,
        Err(e) => return StepResult::failed(failure_from(py, &e)),
    };

    let outcome = with_captured_output(py, &loaded.stream, || {
        let game = loaded
            .make_game
            .call1(py, (spawn.team.game.as_str(), &raw))?;
        loaded.step_fn.call1(py, (game,))?;
        Ok(())
    });
    raw.get().deactivate();
    let stdout = loaded.stream.get().take();

    match outcome {
        Ok(()) => StepResult::ok().with_stdout(stdout),
        Err(e) => StepResult::failed(failure_from(py, &e)).with_stdout(stdout),
    }
}

/// Runs `f` with `sys.stdout` and `sys.stderr` redirected into `stream`.
fn with_captured_output<T>(
    py: Python<'_>,
    stream: &Py<CapturedStream>,
    f: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let sys = PyModule::import(py, "sys")?;
    let saved = (sys.getattr("stdout")?, sys.getattr("stderr")?);
    sys.setattr("stdout", stream)?;
    sys.setattr("stderr", stream)?;
    let result = f();
    let _ = sys.setattr("stdout", saved.0);
    let _ = sys.setattr("stderr", saved.1);
    result
}

pub(super) fn failure_from(py: Python<'_>, err: &PyErr) -> BotFailure {
    let kind = err
        .get_type(py)
        .qualname()
        .map(|q| q.to_string())
        .unwrap_or_else(|_| "Exception".into());
    let message = err
        .value(py)
        .str()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let traceback = err
        .traceback(py)
        .and_then(|tb| tb.format().ok())
        .unwrap_or_default();
    BotFailure::exception(kind, message).with_traceback(traceback)
}
