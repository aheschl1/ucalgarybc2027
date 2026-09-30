//! Bots share the snapshot's memory. Without a copy-on-write image, wasmtime copies the
//! whole snapshot into every bot (about 11 MiB each) and nothing else fails. Its own
//! test binary, so no other test allocates while it measures.
#![cfg(target_os = "linux")]

use std::path::Path;

use ucbc_wasm::{Guest, Outcome, Runtime};

fn private_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/smaps_rollup")
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("Private_"))
        .map(|l| l.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap() * 1024)
        .sum()
}

#[test]
fn bots_share_the_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = Runtime::new(&root.join("../ucbc-cli/python/ucbc")).unwrap();
    let bot = root.join("tests/bots/multi");
    let cache = tempfile::tempdir().unwrap();
    Guest::compile(&runtime, &bot, cache.path(), 256 << 20, 500_000_000).unwrap();
    let before = private_bytes();
    let bots: Vec<Guest> = (0..30)
        .map(|seed| {
            let mut guest = Guest::new(&runtime, &bot, cache.path(), seed, 256 << 20).unwrap();
            let run = guest
                .load(500_000_000, |m| {
                    let m: serde_json::Value =
                        serde_pickle::from_slice(&m, serde_pickle::DeOptions::new()).unwrap();
                    let reply = match m.get("ready") {
                        Some(_) => serde_json::json!({ "identity": {
                            "bot_id": seed, "team": 0, "team_name": "a", "seed": seed, "game": "tictactoe"
                        }}),
                        None => serde_json::Value::Null,
                    };
                    serde_pickle::to_vec(&reply, serde_pickle::SerOptions::new()).unwrap()
                })
                .unwrap();
            assert_eq!(run.outcome, Outcome::Returned);
            guest
        })
        .collect();
    let each = (private_bytes() - before) / bots.len() as u64;
    assert!(
        each < 4 << 20,
        "{} KiB of private memory per bot",
        each >> 10
    );
}
