//! Bots in `tests/bots` against the built snapshot (`make runtime`), answered by a
//! stand-in for the engine.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};
use serde_pickle::{DeOptions, SerOptions};
use tempfile::TempDir;
use ucbc_wasm::{Guest, Outcome, Runtime};

const LOAD_FUEL: u64 = 500_000_000;
const STEP_FUEL: u64 = 500_000_000;
const MEMORY: u64 = 256 << 20;

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ucbc-cli/python/ucbc");
        Runtime::new(&package).expect("no snapshot: run `make runtime`")
    })
}

fn bot(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/bots")
        .join(name)
}

/// Answers the guest the way the engine does and keeps what it reports.
struct Harness {
    /// The team's bytecode cache, the engine's to keep for as long as its bots run.
    cache: TempDir,
    loaded: Option<Value>,
    done: Vec<Value>,
}

impl Harness {
    fn new() -> Self {
        Self {
            cache: tempfile::tempdir().unwrap(),
            loaded: None,
            done: Vec::new(),
        }
    }

    fn answer(&mut self, message: Value) -> Value {
        let (kind, payload) = message.as_object().unwrap().iter().next().unwrap();
        match kind.as_str() {
            "ready" => json!({ "identity": {
                "bot_id": 1, "team": 0, "team_name": "a", "seed": 7, "game": "tictactoe"
            }}),
            "query" => json!({ "ok": payload }),
            "loaded" => {
                self.loaded = Some(payload.clone());
                Value::Null
            }
            "done" => {
                self.done.push(payload.clone());
                Value::Null
            }
            _ => panic!("unexpected message {message}"),
        }
    }

    fn stdout(&self, step: usize) -> &str {
        self.done[step]["stdout"].as_str().unwrap()
    }
}

/// The wire format `ucbc-cli` speaks.
fn decode(message: Vec<u8>) -> Value {
    serde_pickle::from_slice(&message, DeOptions::new()).unwrap()
}

fn encode(reply: Value) -> Vec<u8> {
    serde_pickle::to_vec(&reply, SerOptions::new()).unwrap()
}

/// A bot loaded from source: nothing compiled into the cache.
fn loaded(name: &str, memory: u64) -> (Guest, Harness) {
    let mut harness = Harness::new();
    let (guest, _) = load(&bot(name), &mut harness, memory);
    (guest, harness)
}

/// Loads a bot with what `harness` holds in its cache, and returns the fuel it took.
fn load(dir: &Path, harness: &mut Harness, memory: u64) -> (Guest, u64) {
    let mut guest = Guest::new(runtime(), dir, harness.cache.path(), 7, memory).unwrap();
    let run = guest
        .load(LOAD_FUEL, |m| encode(harness.answer(decode(m))))
        .unwrap();
    assert_eq!(run.outcome, Outcome::Returned);
    assert_eq!(
        harness.loaded,
        Some(Value::Null),
        "load failed: {:?}",
        harness.loaded
    );
    harness.loaded = None;
    (guest, run.fuel)
}

fn compile(dir: &Path, harness: &Harness) {
    let outcome = Guest::compile(runtime(), dir, harness.cache.path(), MEMORY, LOAD_FUEL).unwrap();
    assert_eq!(outcome, Outcome::Returned);
}

fn step(guest: &mut Guest, harness: &mut Harness, tick: u32) -> ucbc_wasm::Run {
    let run = guest
        .step(0, tick, STEP_FUEL, |m| encode(harness.answer(decode(m))))
        .unwrap();
    assert_eq!(run.outcome, Outcome::Returned);
    run
}

#[test]
fn a_multi_file_bot_imports_its_modules_and_keeps_state() {
    let (mut guest, mut harness) = loaded("multi", MEMORY);
    for tick in 0..3 {
        assert_eq!(
            step(&mut guest, &mut harness, tick).outcome,
            Outcome::Returned
        );
    }
    assert_eq!(harness.stdout(0), "1 [1] 10 {'tick': 0}\n");
    assert_eq!(harness.stdout(2), "3 [1, 2, 3] 30 {'tick': 2}\n");
    assert!(harness.done.iter().all(|d| d["error"].is_null()));
}

#[test]
fn a_step_out_of_fuel_is_suspended_and_resumed() {
    let (mut guest, mut harness) = loaded("spin", MEMORY);
    let budget = 10_000_000;
    let mut outcomes = Vec::new();
    for tick in 0..20 {
        let run = guest
            .step(0, tick, budget, |m| encode(harness.answer(decode(m))))
            .unwrap();
        if run.outcome == Outcome::OutOfFuel {
            assert_eq!(run.fuel, budget);
        }
        outcomes.push(run.outcome);
        if harness.done.len() == 2 {
            break;
        }
    }
    assert_eq!(outcomes[0], Outcome::OutOfFuel);
    assert!(outcomes.contains(&Outcome::Returned));
    assert_eq!(harness.done.len(), 2, "{outcomes:?}");
}

#[test]
fn the_same_seed_uses_the_same_fuel_and_randomness() {
    let run = || {
        let (mut guest, mut harness) = loaded("spin", MEMORY);
        let fuel: Vec<u64> = (0..3)
            .map(|t| step(&mut guest, &mut harness, t).fuel)
            .collect();
        (fuel, harness.done)
    };
    assert_eq!(run(), run());
}

#[test]
fn memory_past_the_limit_is_a_memory_error_the_bot_survives() {
    let (mut guest, mut harness) = loaded("hog", 96 << 20);
    step(&mut guest, &mut harness, 0);
    step(&mut guest, &mut harness, 1);
    assert!(harness.stdout(0).starts_with("MemoryError after"));
    assert!(harness.stdout(1).starts_with("MemoryError after"));
    assert!(guest.memory_bytes().unwrap() <= 96 << 20);
}

#[test]
fn python_recursion_is_an_error_and_c_recursion_a_trap() {
    let (mut guest, mut harness) = loaded("deep", MEMORY);
    step(&mut guest, &mut harness, 0);
    assert_eq!(harness.stdout(0), "python recursion\n");
    let trap = guest
        .step(0, 1, u64::MAX / 4, |m| encode(harness.answer(decode(m))))
        .unwrap_err();
    assert!(
        format!("{trap:?}").contains("call stack exhausted"),
        "{trap:?}"
    );
}

#[test]
fn a_bot_sees_only_its_sandbox() {
    let (mut guest, mut harness) = loaded("sandbox", MEMORY);
    step(&mut guest, &mut harness, 0);
    let out = harness.stdout(0);
    let clock: u64 = out.lines().next().unwrap()["clock ".len()..]
        .parse()
        .unwrap();
    assert!(clock > 0, "{out}");
    assert!(
        out.contains("write PermissionError") || out.contains("write OSError"),
        "{out}"
    );
    assert!(
        out.contains("outside FileNotFoundError") || out.contains("outside PermissionError"),
        "{out}"
    );
    assert!(out.contains("sleep OSError"), "{out}");
    let (mut again, mut harness_again) = loaded("sandbox", MEMORY);
    step(&mut again, &mut harness_again, 0);
    assert_eq!(
        out,
        harness_again.stdout(0),
        "the clock and randomness repeat"
    );
}

#[test]
fn compiled_bytecode_loads_on_a_fraction_of_the_fuel() {
    // A bot big enough that compiling it is most of loading it.
    let dir = tempfile::tempdir().unwrap();
    let mut source = String::new();
    for i in 0..300 {
        source += &format!("def f{i}(x):\n    y = x * {i} + 1\n    return y - x\n\n");
    }
    source += "def step(handle):\n    pass\n";
    std::fs::write(dir.path().join("main.py"), source).unwrap();
    let mut harness = Harness::new();
    let (_, from_source) = load(dir.path(), &mut harness, MEMORY);
    compile(dir.path(), &harness);
    assert!(
        harness
            .cache
            .path()
            .join("bot/main.cpython-314.pyc")
            .is_file()
    );
    let (_, from_bytecode) = load(dir.path(), &mut harness, MEMORY);
    assert!(
        from_bytecode * 10 < from_source,
        "{from_bytecode} fuel from bytecode, {from_source} from source"
    );
}

#[test]
fn a_module_that_does_not_compile_is_the_bots_error() {
    let mut harness = Harness::new();
    compile(&bot("broken"), &harness);
    let mut guest = Guest::new(runtime(), &bot("broken"), harness.cache.path(), 7, MEMORY).unwrap();
    guest
        .load(LOAD_FUEL, |m| encode(harness.answer(decode(m))))
        .unwrap();
    assert_eq!(harness.loaded.unwrap()["kind"], "SyntaxError");
}

#[test]
fn load_reports_a_missing_step() {
    let mut harness = Harness::new();
    let mut guest = Guest::new(runtime(), &bot("nostep"), harness.cache.path(), 7, MEMORY).unwrap();
    guest
        .load(LOAD_FUEL, |m| encode(harness.answer(decode(m))))
        .unwrap();
    let failure = harness.loaded.unwrap();
    assert_eq!(failure["kind"], "AttributeError");
    assert_eq!(failure["message"], "main.py must define step(handle)");
}

#[test]
fn a_load_that_never_ends_runs_out_of_fuel() {
    let mut harness = Harness::new();
    let mut guest =
        Guest::new(runtime(), &bot("slowload"), harness.cache.path(), 7, MEMORY).unwrap();
    let run = guest
        .load(10_000_000, |m| encode(harness.answer(decode(m))))
        .unwrap();
    assert_eq!(run.outcome, Outcome::OutOfFuel);
    assert!(harness.loaded.is_none());
}
