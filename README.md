# UCBC

UCalgary Battlecode. Python bots battle, managed by a rust game engine.

## Layout

| Path | What |
| --- | --- |
| `ucbc-engine/` | Game-agnostic match engine. |
| `ucbc-tictactoe/` | Example tic-tac-toe game implementation. |
| `ucbc-py/` | The `ucbc._engine` extension module + Python bot runtime. |
| `ucbc-sdk/` | The `ucbc` Python package: bot API, classes per game, `ucbc run`. |
| `ucbc-cli/` | `ucbc-dev`, a Rust-only binary for engine work & testing. |
| `bots/<game>/` | Sample bots. Consume the ucbc-sdk and are players. |

## Setup

Rust 1.85+, Python 3.12, and [uv](https://docs.astral.sh/uv/).

```bash
uv sync                 # builds the extension into .venv
make test               # rust and python tests
make lint               # linting + type checking python & rust 
```

After changing Rust code, rebuild the extension with `make dev`.

## Running a match

```bash
uv run ucbc run bots/tictactoe/random bots/tictactoe/first_empty
uv run ucbc run bots/tictactoe/random bots/tictactoe/first_empty --seed 7 --sets 5
uv run ucbc run bots/tictactoe/random bots/tictactoe/first_empty --show-bot-output
uv run ucbc run bots/tictactoe/random bots/tictactoe/random --replay r.json --summary s.json
```

Rust-only game: `cargo run -p ucbc-cli -- run --a first-empty --b random`.

## Writing a bot

A bot is a directory with a `main.py` that defines `step`. The engine calls it once per step with a handle.

```python
# bots/tictactoe/mine/main.py
from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    if (1, 1) in handle.empty_cells():
        handle.place(1, 1)
    else:
        handle.place(*handle.empty_cells()[0])
```

```bash
uv run ucbc run bots/tictactoe/mine bots/tictactoe/random
```

## Adding a game

Copy the shape of tic-tac-toe.

1. A crate `ucbc-foo/` implementing `ucbc_engine::Game`, added to the workspace.
2. In `ucbc-py/Cargo.toml`, an optional dependency and a feature `foo = ["dep:ucbc-foo"]`
   in `default`; in `ucbc-py/src/lib.rs`, `registry.register::<ucbc_foo::Foo>()` under
   `#[cfg(feature = "foo")]`.
3. `ucbc-sdk/ucbc/games/foo.py`: a subclass of `ucbc.handle.Handle` with typed methods over
   `self._query` and `self._act`, ending with `HANDLE = FooHandle`.
4. Bots under `bots/foo/`.

`make dev`, then `uv run ucbc run bots/foo/a bots/foo/b`. A wheel with one game only:
`maturin build --no-default-features -F foo`.

## Engine model

A match is sets; a set is ticks; a tick is one run of the game's schedule; a step is
one bot's turn within it. A team is one code submission and may own many bots; the
game decides which bots exist, who owns them, and the step order. Each bot runs on
its own thread with its own copy of the team's `main.py`, stepped serially. A bot's
runtime failure is reported to the game, which decides the consequence.
