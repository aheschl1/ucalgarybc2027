# UCBC

UCalgary Battlecode: Python bots, Rust engine. Work tracked in [Linear](https://linear.app/andrew-heschl/team/UCBC/overview).

## Setup

Rust 1.85+, Python 3.12, [uv](https://docs.astral.sh/uv/), Node 20.19+.

```bash
uv sync         # builds the engine extension into .venv
make viewer     # builds the replay viewer
make test
make lint
```

`make dev` after Rust changes.

## Run a match

```bash
uv run ucbc run bots/tictactoe/random bots/tictactoe/first_empty --view
```

| Flag | |
| --- | --- |
| `--seed N`, `--sets N` | |
| `--step-ms`, `--memory-mb` | per bot; default 500 ms, 1 GiB ([limits](docs/resourcelimits.md)) |
| `--show-bot-output` | bot stdout/stderr |
| `--replay FILE`, `--summary FILE` | write JSON |
| `--view` | open replay in browser |

`uv run ucbc view r.json` replays a saved file.

## Write a bot

A directory with a `main.py` defining `step`.

```python
# bots/tictactoe/mine/main.py
from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
```

```bash
uv run ucbc run bots/tictactoe/mine bots/tictactoe/random
```

## Platform API

```bash
cp .env.example .env && cp .env.local.example .env.local
make up                                 # Postgres + API in Docker on :8000, /docs for routes
uv run ucbc-api-cli create-admin
make test-api
```

`make db` + `make migrate` + `make api` runs the API from the checkout instead. `ENV=prod`
reads `.env.prod` over `.env.local`. Migrations: `uv run alembic revision -m "..."`, raw SQL in `op.execute`.

## Layout

| Path | |
| --- | --- |
| `ucbc-engine/` | game-agnostic engine |
| `ucbc-tictactoe/` | example game + its viewer renderer |
| `ucbc-py/` | `ucbc` wheel: extension, bot runtime, CLI |
| `ucbc-sdk/` | `ucbc` package bots import |
| `ucbc-cli/` | `ucbc-dev`, Rust-only dev binary |
| `ucbc-viewer/` | replay viewer (TS, Vite) |
| `ucbc-api/` | platform API (FastAPI, Postgres) |
| `ucbc-web/` | platform frontend (React, Vite) |
| `bots/<game>/` | sample bots |
| `tests/` | Python integration tests |

Engine internals and adding a game: [docs/engine.md](docs/engine.md).
