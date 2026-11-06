//! `ucbc_rl._native`: `FrontierEnv`, Frontier sets for training with one policy unit
//! waiting in each env. Arrays cross as bytes in native byte order, a row per env, for
//! `numpy.frombuffer`.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use serde_json::{Value, json};
use ucbc_engine::{BotResourceLimit, GameRegistry, TeamSpec};
use ucbc_frontier::{Config, Frontier};
use ucbc_wasm::{PyTeam, Runtime};

use crate::frontier::{Codec, Context, HEADS, VECTOR, plane};
use crate::{Batch, EnvSpec, Event, Recording, Seat, VecEnv};

fn to_pyerr(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<FrontierEnv>()
}

#[pyclass(module = "ucbc_rl")]
pub struct FrontierEnv {
    inner: Mutex<Inner>,
    codec: Codec,
    #[pyo3(get)]
    num_envs: usize,
    /// Choices per action head.
    #[pyo3(get)]
    heads: Vec<usize>,
    /// Planes, rows, columns of one env's grid.
    #[pyo3(get)]
    grid_shape: (usize, usize, usize),
    #[pyo3(get)]
    vector_len: usize,
}

struct Inner {
    env: VecEnv,
    /// Where each env's waiting unit was encoded.
    at: Vec<Context>,
}

/// One batch, before it becomes Python objects.
struct Out {
    grid: Vec<u8>,
    vector: Vec<f32>,
    agent: Vec<i64>,
    team: Vec<i64>,
    reward: Vec<f32>,
    events: Vec<Event>,
}

#[pymethods]
impl FrontierEnv {
    /// One env per entry of `opponents`: `None` for self-play, or a Python team's directory.
    /// The policy plays team 0 in even envs and team 1 in odd ones.
    #[new]
    #[pyo3(signature = (opponents, *, game_config = None, max_ticks = 2000, seed = 0, step_ms = 50, memory_bytes = 256 << 20, record_dir = None, record_every = 100))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        opponents: Vec<Option<PathBuf>>,
        game_config: Option<&str>,
        max_ticks: u32,
        seed: u64,
        step_ms: u64,
        memory_bytes: u64,
        record_dir: Option<PathBuf>,
        record_every: u32,
    ) -> PyResult<Self> {
        let game_config: Option<Value> = game_config
            .map(serde_json::from_str)
            .transpose()
            .map_err(to_pyerr)?;
        let vision = Config::from(game_config.as_ref()).map_err(to_pyerr)?.vision;
        let limits = BotResourceLimit::new(step_ms, memory_bytes);
        let runtime = match opponents.iter().any(Option::is_some) {
            true => {
                let init: PathBuf = py.import("ucbc")?.getattr("__file__")?.extract()?;
                let package = init.parent().unwrap_or(&init);
                Some(Arc::new(Runtime::new(package).map_err(to_pyerr)?))
            }
            false => None,
        };
        let mut teams: HashMap<PathBuf, Arc<PyTeam>> = HashMap::new();
        let mut specs = Vec::with_capacity(opponents.len());
        for (i, opponent) in opponents.into_iter().enumerate() {
            let other = match opponent {
                None => Seat::Policy,
                Some(dir) => {
                    let team = match teams.entry(dir.clone()) {
                        Entry::Occupied(e) => e.get().clone(),
                        Entry::Vacant(e) => {
                            let rt = runtime.clone().unwrap();
                            let team = PyTeam::read(rt, &dir, "frontier", &limits)
                                .map_err(|f| PyValueError::new_err(f.detail()))?;
                            e.insert(Arc::new(team)).clone()
                        }
                    };
                    let name = dir.display().to_string();
                    Seat::Team(Arc::new(move || {
                        let team = team.clone();
                        TeamSpec::new(name.clone(), Box::new(move |ctx| team.spawn(ctx)))
                    }))
                }
            };
            let seats = match i % 2 {
                0 => vec![Seat::Policy, other],
                _ => vec![other, Seat::Policy],
            };
            specs.push(EnvSpec {
                game: "frontier".into(),
                game_config: game_config.clone(),
                max_ticks,
                limits,
                seats,
                seed,
                observe: json!({"type": "observe"}),
                record: record_dir.clone().map(|dir| Recording {
                    dir,
                    every: record_every,
                }),
            });
        }
        let mut registry = GameRegistry::new();
        registry.register::<Frontier>();
        let env = VecEnv::new(Arc::new(registry), specs).map_err(to_pyerr)?;
        let codec = Codec { vision, max_ticks };
        let side = codec.side();
        Ok(Self {
            num_envs: env.num_envs(),
            inner: Mutex::new(Inner {
                env,
                at: Vec::new(),
            }),
            codec,
            heads: HEADS.to_vec(),
            grid_shape: (plane::COUNT, side, side),
            vector_len: VECTOR,
        })
    }

    /// The first unit waiting in each env.
    fn reset<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = py.detach(|| {
            let mut inner = self.inner.lock().unwrap();
            let batch = inner.env.reset().map_err(|e| e.to_string())?;
            self.out(&mut inner, batch)
        });
        to_dict(py, out.map_err(to_pyerr)?)
    }

    /// `actions`: int64, `heads` ids per env.
    fn step<'py>(&self, py: Python<'py>, actions: &[u8]) -> PyResult<Bound<'py, PyDict>> {
        let out = py.detach(|| {
            let mut inner = self.inner.lock().unwrap();
            let ids = ids(actions, self.num_envs)?;
            let actions = inner
                .at
                .iter()
                .zip(ids)
                .map(|(at, ids)| at.decode(&ids))
                .collect();
            let batch = inner.env.step(actions).map_err(|e| e.to_string())?;
            self.out(&mut inner, batch)
        });
        to_dict(py, out.map_err(to_pyerr)?)
    }
}

impl FrontierEnv {
    fn out(&self, inner: &mut Inner, batch: Batch) -> Result<Out, String> {
        let n = batch.turns.len();
        let mut out = Out {
            grid: vec![0; n * self.codec.grid_len()],
            vector: vec![0.0; n * VECTOR],
            agent: Vec::with_capacity(3 * n),
            team: Vec::with_capacity(n),
            reward: Vec::with_capacity(3 * n),
            events: batch.events,
        };
        let rows = out
            .grid
            .chunks_mut(self.codec.grid_len())
            .zip(out.vector.chunks_mut(VECTOR));
        inner.at.clear();
        for (turn, (grid, vector)) in batch.turns.iter().zip(rows) {
            inner.at.push(self.codec.encode(&turn.obs, grid, vector)?);
            let (a, r) = (turn.agent, turn.reward);
            out.agent
                .extend([a.env as i64, i64::from(a.episode), a.bot.0 as i64]);
            out.team.push(i64::from(turn.team.0));
            out.reward.extend([r.own, r.team, r.result]);
        }
        Ok(out)
    }
}

/// `HEADS.len()` ids per env, each in range for its head.
fn ids(actions: &[u8], envs: usize) -> Result<Vec<[usize; 9]>, String> {
    if actions.len() != envs * HEADS.len() * 8 {
        return Err(format!(
            "{} bytes of actions for {envs} envs of {} int64 ids",
            actions.len(),
            HEADS.len()
        ));
    }
    let ids: Vec<i64> = actions
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| i64::from_ne_bytes(*b))
        .collect();
    ids.chunks(HEADS.len())
        .enumerate()
        .map(|(env, row)| {
            let mut out = [0; 9];
            for (head, (&id, o)) in row.iter().zip(&mut out).enumerate() {
                *o = usize::try_from(id)
                    .ok()
                    .filter(|&id| id < HEADS[head])
                    .ok_or_else(|| format!("env {env}: head {head} id {id} is out of range"))?;
            }
            Ok(out)
        })
        .collect()
}

fn bytes<const N: usize, T: Copy>(values: &[T], to: fn(T) -> [u8; N]) -> Vec<u8> {
    values.iter().flat_map(|&v| to(v)).collect()
}

fn to_dict(py: Python<'_>, out: Out) -> PyResult<Bound<'_, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("grid", PyBytes::new(py, &out.grid))?;
    dict.set_item(
        "vector",
        PyBytes::new(py, &bytes(&out.vector, f32::to_ne_bytes)),
    )?;
    dict.set_item(
        "agent",
        PyBytes::new(py, &bytes(&out.agent, i64::to_ne_bytes)),
    )?;
    dict.set_item(
        "team",
        PyBytes::new(py, &bytes(&out.team, i64::to_ne_bytes)),
    )?;
    dict.set_item(
        "reward",
        PyBytes::new(py, &bytes(&out.reward, f32::to_ne_bytes)),
    )?;
    let (mut gone, mut set_ends, mut rejected) = (Vec::new(), Vec::new(), Vec::new());
    for event in out.events {
        match event {
            Event::Gone {
                agent: a,
                reward: r,
            } => {
                gone.push((a.env, a.episode, a.bot.0, r.own, r.team, r.result));
            }
            Event::SetEnd {
                env,
                episode,
                result,
            } => {
                let winner = result.winner_team.map(|t| t.0);
                set_ends.push((env, episode, winner, result.detail));
            }
            Event::Acted { agent: a, replies } => {
                let n = replies.iter().filter(|r| r.is_err()).count();
                rejected.push((a.env, a.episode, a.bot.0, n, replies.len()));
            }
        }
    }
    dict.set_item("gone", gone)?;
    dict.set_item("set_ends", set_ends)?;
    dict.set_item("rejected", rejected)?;
    Ok(dict)
}
