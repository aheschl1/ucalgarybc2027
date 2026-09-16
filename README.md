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
make up                                 # Postgres, MinIO (:9001 console), API + web on :8000 (/api/docs), worker
uv run ucbc-api-cli create-admin
make test-api
```

`POST /submissions` uploads a zipped bot (main.py at the top, 1 MiB) to blob storage;
`POST /matches/queue` queues a match between two bot directories on the worker host;
`make worker` plays queued matches from the checkout, `UCBC_WORKER_SLOTS` at a time.

`make db` + `make migrate` + `make api` runs the API from the checkout instead, and
`npm run dev -w ucbc-web` serves the frontend on :5173 with `/api` proxied to it; `make web`
builds the frontend into the API package. `ENV=prod` reads `.env.prod` over `.env.local`. Migrations: `uv run alembic revision -m "..."`, raw SQL in `op.execute`.

## Layout

| Path | Builds | Reached as |
| --- | --- | --- |
| `ucbc-engine/` | crate: game-agnostic engine | `use ucbc_engine` |
| `ucbc-tictactoe/` | crate: example game + its viewer renderer | registered in `ucbc-py` and `ucbc-dev` |
| `ucbc-py/` | pip dist `ucbc`: extension, bot process, CLI | `uv run ucbc`, `import ucbc_engine` |
| `ucbc-sdk/` | pip dist `ucbc-sdk` | `import ucbc` inside a bot |
| `ucbc-dev/` | cargo bin `ucbc-dev`, Rust only | `cargo run -p ucbc-dev`, `make sdk` |
| `ucbc-viewer/` | replay viewer (TS, Vite) | `ucbc view`, `make viewer` |
| `ucbc-api/` | pip dist `ucbc-api` (FastAPI, Postgres) | `uv run ucbc-api`, `uv run ucbc-api-cli` |
| `ucbc-worker/` | pip dist `ucbc-worker` | `uv run ucbc-worker` |
| `ucbc-web/` | platform frontend (React, Vite) | served by `ucbc-api` at `/`, `make web` |

`pip install ucbc` gives both the command and the SDK. Bots import `ucbc`; the runner
imports `ucbc_engine`.
| `bots/<game>/` | sample bots |
| `tests/` | Python integration tests |

Engine internals and adding a game: [docs/engine.md](docs/engine.md). The API, queue, and
worker: [docs/platform.md](docs/platform.md).
