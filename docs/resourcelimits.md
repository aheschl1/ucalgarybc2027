# Resource limits

Every match says what one bot may use; games have no say. The engine carries it to
the bot's runtime, which enforces it, and records it in the replay.

```rust
// ucbc-engine: bot/mod.rs
pub struct BotResourceLimit {
    pub step_ms: u64,          // wall clock, per step and for loading main.py
    pub memory_bytes: u64,     // the bot process's address space
}

MatchConfig::new("tictactoe", 3, seed, 2, BotResourceLimit::new(50, 256 << 20))
```

```bash
uv run ucbc run a/ b/ --step-ms 50 --memory-mb 256          # defaults: 500 ms, 1024 MiB
```

```python
run_match("a", "b", step_ms=50, memory_bytes=256 * 2**20)   # defaults in ucbc_engine.runner
```

The engine enforces nothing itself; the Python bot process does. Rust test bots are unlimited.

## Time

The clock is wall-clock time while the bot has control: from sending `step` until
`done` comes back, minus the time the engine spends answering the bot's queries and
actions (`StepCtx::engine_time`). Loading `main.py` gets its own budget of the same
size; Python start-up before `ready` is engine time (up to `STARTUP`, 10 s).

A bot past its deadline is stopped, not interrupted: the runner sends `SIGSTOP`. The
step ends there: whatever the bot already did stands, and the replay records an
ordinary step with no failure and `time_us` at the budget. What the game makes of an
empty step is the game's business; tic-tac-toe forfeits "did not place a mark".

On the bot's next turn the runner sends `SIGCONT` and the bot carries on where it
stopped; that turn ends when the earlier work returns, so `step` is not called again
until the turn after. An overrun of any length therefore costs two turns. A message
sent just as the bot was stopped is answered on that next turn, so a late action
lands there. A load that overruns is a load failure (`TimeoutError`), not resumed.

## Memory

`_bot.py` sets `RLIMIT_AS` to `memory_bytes` on itself before loading `main.py`, so
the whole address space counts, C extensions included. The allocation that would
cross the limit fails and the bot sees `MemoryError` where it happened; if reporting
it also runs out, the step fails as a crash. Both lose the same way. `memory` in the
replay is the process's resident size, read from `/proc/self/statm` at the end of
each step.

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
  "usage": { "time_us": 531778, "memory": 6583656 },
  "failure": { "kind": "MemoryError", "message": "", "traceback": "..." }
}
```

- `time_us`: wall clock while the bot had control during the step, engine time
  excluded; at the budget for a step that was stopped. Loading is not recorded.
- `memory`: resident bytes when the step returned. Absent when it did not (stopped,
  or the process died) and for Rust bots.
- `failure`: absent for a stopped step; `MemoryError`, or `Crash` when the process died.

Replays are deterministic for a seed except `usage`.
