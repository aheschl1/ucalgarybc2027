# Resource limits

Every match says what one bot may use; games have no say. The engine carries it to
the bot's runtime (`ucbc-wasm`), which enforces it, and records it in the replay.

```rust
// ucbc-engine: bot/mod.rs
pub struct BotResourceLimit {
    pub step_ms: u64,          // bot time, per step and for loading main.py
    pub memory_bytes: u64,     // the bot's linear memory, interpreter included
}

MatchConfig::new("tictactoe", 3, seed, 2, BotResourceLimit::new(50, 256 << 20))
```

```bash
uv run ucbc run a/ b/ --step-ms 50 --memory-mb 256          # defaults: 500 ms, 1024 MiB
```

```python
run_match("a", "b", step_ms=50, memory_bytes=256 * 2**20)   # defaults in ucbc.runner
```

The engine enforces nothing itself; the runtime does. Rust test bots are unlimited.

## Time

Bot time is wasm fuel: one unit per instruction the bot executes, six million to the
millisecond on its clock (`time.perf_counter`), about what a core runs in one. A match
uses the same fuel on every machine. Time the engine spends
answering queries and actions is not metered. Loading `main.py` gets a budget of the
same size.

A bot past its budget is suspended, not interrupted. The step ends there: whatever the
bot already did stands, and the replay records an ordinary step with no failure and
`time_us` at the budget. What the game makes of an empty step is the game's business;
tic-tac-toe forfeits "did not place a mark".

On the bot's next turn the suspended step resumes and that turn ends when it returns,
so `step` is not called again until the turn after. An overrun of any length therefore
costs two turns. A message sent just as the bot was suspended is answered on that next
turn, so a late action lands there. A load that overruns is a load failure
(`TimeoutError`).

## Memory

`memory_bytes` caps the bot's linear memory, the interpreter's own 40 MiB included.
The allocation that would cross it fails and the bot sees `MemoryError` where it
happened; if reporting it also runs out, the step fails as a crash. Both lose the same
way. `memory` in the replay is the linear memory size when the step returned; it grows
and never shrinks, so it is a high-water mark.

## What is recorded

The limits that applied, under the replay's `config`:

```json
"config": { "game": "tictactoe", "sets": 3, "seed": 0, "teams": 2, "max_ticks": 1000,
            "limits": { "step_ms": 500, "memory_bytes": 1073741824 } }
```

Every step:

```json
{
  "bot": 1, "team": 1,
  "actions": [...],
  "usage": { "time_us": 531, "memory": 41943040 },
  "failure": { "kind": "MemoryError", "message": "", "traceback": "..." }
}
```

- `time_us`: fuel the step used, as microseconds of bot time; at the budget for a
  suspended step. Loading is not recorded.
- `memory`: linear memory bytes when the step returned. Absent when it did not
  (suspended) and for Rust bots.
- `failure`: absent for a suspended step; `MemoryError`, or `Crash` when the
  interpreter trapped.

Replays are deterministic for a seed, `usage` included.
