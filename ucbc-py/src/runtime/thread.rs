//! The bot thread: runs `main.py` once into a fresh namespace, then one step per
//! `Step` message.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use super::api::{Api, StepToken};
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
    raw: Py<Api>,
    handle: Py<PyAny>,
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

    while let Some(ToBot::Step(token)) = link.recv() {
        let (result, token) = Python::attach(|py| run_step(py, &loaded, token));
        if link.to_runner.send(FromBot::Done(result, token)).is_err() {
            break;
        }
    }
    Python::attach(|_py| drop(loaded));
}

/// Executes the team's code into a fresh namespace and builds the bot's one handle.
/// Output written while loading stays in the stream and is reported with the first
/// step.
fn load(py: Python<'_>, spawn: &BotSpawn) -> PyResult<Loaded> {
    let stream = Py::new(py, CapturedStream::default())?;
    let make_handle = PyModule::import(py, "ucbc._runtime")?.getattr("make_handle")?;
    let raw = Py::new(
        py,
        Api::new(
            spawn.bot,
            spawn.team_name.clone(),
            spawn.seed,
            spawn.team.game.clone(),
            PyDict::new(py).unbind(),
        ),
    )?;
    let handle = make_handle.call1((spawn.team.game.as_str(), &raw))?;
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
            pyo3::exceptions::PyAttributeError::new_err("main.py must define step(handle)")
        })
    })?;
    Ok(Loaded {
        step_fn: step_fn.unbind(),
        raw,
        handle: handle.unbind(),
        stream,
    })
}

/// Runs one step with the token slotted into the bot's handle, then takes it back.
fn run_step(py: Python<'_>, loaded: &Loaded, token: StepToken) -> (StepResult, StepToken) {
    let raw = loaded.raw.get();
    raw.begin_step(token);
    let outcome = with_captured_output(py, &loaded.stream, || {
        loaded.step_fn.call1(py, (&loaded.handle,))?;
        Ok(())
    });
    let token = raw.end_step().expect("token was slotted in above");
    let stdout = loaded.stream.get().take();

    let result = match outcome {
        Ok(()) => StepResult::ok().with_stdout(stdout),
        Err(e) => StepResult::failed(failure_from(py, &e)).with_stdout(stdout),
    };
    (result, token)
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
