# UCBC

UCalgary Battlecode. Python bots battle, managed by a rust game engine.

<https://linear.app/andrew-heschl/team/UCBC/overview>

## Layout

| Path | What |
| --- | --- |
| `ucbc-engine/` | Game-agnostic match engine. |
| `ucbc-tictactoe/` | Example tic-tac-toe game implementation. |
| `ucbc-py/` | The `ucbc` distribution (`pip install ucbc`): the `_engine` extension, the bot process, `ucbc run`. Depends on `ucbc-sdk`. |
| `ucbc-sdk/` | The `ucbc-sdk` distribution: the `ucbc` package a bot imports, one handle per game. Pure Python. |
| `ucbc-cli/` | `ucbc-dev`, a Rust-only binary for engine work & testing. |
| `ucbc-api/` | The platform API (FastAPI, Postgres) and `ucbc-api-cli`. Migrations are raw SQL under `alembic/`. |
| `bots/<game>/` | Sample bots. |
| `tests/` | Python tests of the whole thing: matches, limits, lockdown; `tests/api/` for the API. |

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
uv run ucbc run bots/tictactoe/random bots/tictactoe/first_empty --step-ms 50 --memory-mb 256
```

Each bot gets 500 ms per step and 1 GiB unless the flags say otherwise; see
[docs/resourcelimits.md](docs/resourcelimits.md).

`--upload` records the match on the platform API: the match is created before the first
set, each set is posted as it ends, and the result marks it done. It needs `UCBC_API_URL`,
`UCBC_API_USERNAME`, and `UCBC_API_PASSWORD` (an admin user); without them the match plays
and a warning says nothing was uploaded.

```bash
UCBC_API_URL=http://127.0.0.1:8000 UCBC_API_USERNAME=admin UCBC_API_PASSWORD=admin \
  uv run ucbc run bots/tictactoe/random bots/tictactoe/first_empty --upload
```

Rust-only game: `cargo run -p ucbc-cli -- run --a first-empty --b random`.

## Platform API

Configuration comes from `.env` plus one realm file: `.env.local` by default, `.env.prod`
when `ENV=prod`. Process variables win over both. `.env.local` holds the local database URL
and the default admin credentials; `.env.prod` is not committed.

```bash
make up                         # Postgres + the API in Docker, migrated, on :8000
make db                         # Postgres only, for running the API from the checkout
make migrate                    # alembic upgrade head
make api                        # uvicorn on 127.0.0.1:8000; /docs shows the routes
uv run ucbc-api-cli create-admin            # from UCBC_ADMIN_USERNAME/PASSWORD
uv run ucbc-api-cli create-user --username bob --password pw
ENV=prod uv run ucbc-api-cli list-users     # any command, against .env.prod
make test-api                   # starts its own Postgres through testcontainers
```

Users authenticate with HTTP Basic; passwords are argon2 hashes. Every match route is admin
only for now. A new migration is `uv run alembic revision -m "..."` with SQL in `op.execute`.

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
3. `make sdk` generates `ucbc-sdk/ucbc/games/foo/_api.py` from the game's Rust types;
   `ucbc-sdk/ucbc/games/foo/__init__.py` subclasses `FooApi` with any conveniences and ends
   with `HANDLE = FooHandle`.
4. Bots under `bots/foo/`.

`make dev`, then `uv run ucbc run bots/foo/a bots/foo/b`. Wheels go to `dist/` with
`make wheels`; `make wheels GAME=foo` (or `make dev GAME=foo`) builds the engine with only
that game and the SDK with only its handle.

## Engine model

A match is sets; a set is ticks; a tick is one run of the game's schedule; a step is
one bot's turn within it. A team is one code submission and may own many bots; the
game decides which bots exist, who owns them, and the step order. Each bot runs in
its own locked-down process with its own copy of the team's `main.py`, stepped
serially, within the time and memory the match sets. A bot's runtime
failure, including running out of either, is reported to the game, which decides the
consequence. Every recorded step carries the time and memory it used.
