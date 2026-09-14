use std::sync::{Arc, Mutex};

use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use super::{FromBot, Link, ToBot};
use ucbc_engine::{ActionError, BotRef};

pyo3::create_exception!(
    _engine,
    PyQueryError,
    PyException,
    "The game refused a query."
);
pyo3::create_exception!(
    _engine,
    PyActionError,
    PyException,
    "The game refused an action."
);
pyo3::create_exception!(
    _engine,
    PySetOver,
    PyActionError,
    "The set is already over."
);

/// The one way to reach the game during a step. Not `Clone`: the runner creates one
/// per step, the bot thread holds it while the bot runs, and it goes back with `Done`.
/// Outside a step no token exists, so a stashed handle cannot reach the engine.
pub(super) struct StepToken {
    link: Arc<Link>,
    set_index: u32,
    tick: u32,
}

impl StepToken {
    pub(super) fn new(link: Arc<Link>, set_index: u32, tick: u32) -> Self {
        Self {
            link,
            set_index,
            tick,
        }
    }
}

/// The raw, game-agnostic handle a bot's Python code receives. One per bot; the
/// engine slots a [`StepToken`] into it for the duration of each step.
#[pyclass(module = "ucbc._engine", name = "RawHandle", frozen)]
pub struct Api {
    bot: BotRef,
    #[pyo3(get)]
    team_name: String,
    #[pyo3(get)]
    seed: u64,
    #[pyo3(get)]
    game: String,
    memory: Py<PyDict>,
    token: Mutex<Option<StepToken>>,
}

impl Api {
    pub(super) fn new(
        bot: BotRef,
        team_name: String,
        seed: u64,
        game: String,
        memory: Py<PyDict>,
    ) -> Self {
        Self {
            bot,
            team_name,
            seed,
            game,
            memory,
            token: Mutex::new(None),
        }
    }

    pub(super) fn begin_step(&self, token: StepToken) {
        *self.token.lock().expect("token lock") = Some(token);
    }

    /// Takes the token back. `None` only if `begin_step` was never called.
    pub(super) fn end_step(&self) -> Option<StepToken> {
        self.token.lock().expect("token lock").take()
    }

    /// Runs `f` with the step's token, or fails if no step is in progress. The lock
    /// is held throughout, so calls from a bot's own threads serialize.
    fn with_token<T>(&self, f: impl FnOnce(&StepToken) -> Option<T>) -> PyResult<T> {
        let guard = self.token.lock().expect("token lock");
        let token = guard
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no step in progress for this handle"))?;
        f(token).ok_or_else(|| PyRuntimeError::new_err("engine link closed"))
    }

    /// One round trip to the runner. Holds the token for the whole exchange, so calls
    /// from a bot's own threads serialize instead of crossing replies.
    fn exchange(&self, py: Python<'_>, msg: FromBot) -> PyResult<ToBot> {
        let guard = self.token.lock().expect("token lock");
        let token = guard
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("no step in progress for this handle"))?;
        py.detach(|| {
            let receiver = token.link.from_runner.lock().ok()?;
            token.link.to_runner.send(msg).ok()?;
            receiver.recv().ok()
        })
        .ok_or_else(|| PyRuntimeError::new_err("engine link closed"))
    }
}

fn parse(json: &str) -> PyResult<Value> {
    serde_json::from_str(json).map_err(|e| PyValueError::new_err(format!("invalid JSON: {e}")))
}

#[pymethods]
impl Api {
    #[getter]
    fn bot_id(&self) -> u64 {
        self.bot.id.0
    }

    #[getter]
    fn team(&self) -> u32 {
        self.bot.team.0
    }

    #[getter]
    fn set_index(&self) -> PyResult<u32> {
        self.with_token(|t| Some(t.set_index))
    }

    #[getter]
    fn tick(&self) -> PyResult<u32> {
        self.with_token(|t| Some(t.tick))
    }

    #[getter]
    fn memory(&self, py: Python<'_>) -> Py<PyDict> {
        self.memory.clone_ref(py)
    }

    /// Send a JSON query; returns the JSON response.
    fn query(&self, py: Python<'_>, json: &str) -> PyResult<String> {
        let query = parse(json)?;
        match self.exchange(py, FromBot::Query(query))? {
            ToBot::QueryReply(Ok(v)) => Ok(v.to_string()),
            ToBot::QueryReply(Err(e)) => Err(PyQueryError::new_err(e.to_string())),
            _ => Err(PyRuntimeError::new_err("engine protocol error")),
        }
    }

    /// Send a JSON action; returns the JSON response.
    fn act(&self, py: Python<'_>, json: &str) -> PyResult<String> {
        let action = parse(json)?;
        match self.exchange(py, FromBot::Act(action))? {
            ToBot::ActReply(Ok(v)) => Ok(v.to_string()),
            ToBot::ActReply(Err(ActionError::SetOver)) => {
                Err(PySetOver::new_err(ActionError::SetOver.to_string()))
            }
            ToBot::ActReply(Err(e)) => Err(PyActionError::new_err(e.to_string())),
            _ => Err(PyRuntimeError::new_err("engine protocol error")),
        }
    }
}
