use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// The raw, game-agnostic handle a bot's Python code receives. `ucbc._runtime` wraps
/// it in the typed class for the game being played.
#[pyclass(module = "ucbc._engine", name = "RawGame", frozen)]
pub struct Api {
    link: Arc<Link>,
    bot: BotRef,
    #[pyo3(get)]
    team_name: String,
    #[pyo3(get)]
    set_index: u32,
    #[pyo3(get)]
    tick: u32,
    #[pyo3(get)]
    seed: u64,
    #[pyo3(get)]
    game: String,
    memory: Py<PyDict>,
    active: AtomicBool,
}

impl Api {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        link: Arc<Link>,
        bot: BotRef,
        team_name: String,
        set_index: u32,
        tick: u32,
        seed: u64,
        game: String,
        memory: Py<PyDict>,
    ) -> Self {
        Self {
            link,
            bot,
            team_name,
            set_index,
            tick,
            seed,
            game,
            memory,
            active: AtomicBool::new(true),
        }
    }

    /// The step is over; a stashed handle must not reach the engine.
    pub(super) fn deactivate(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    fn exchange(&self, py: Python<'_>, msg: FromBot) -> PyResult<ToBot> {
        if !self.active.load(Ordering::SeqCst) {
            return Err(PyRuntimeError::new_err(
                "this api handle belongs to a finished step",
            ));
        }
        py.detach(|| {
            let receiver = self.link.from_runner.lock().ok()?;
            self.link.to_runner.send(msg).ok()?;
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
