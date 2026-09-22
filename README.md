# UCBC

UCalgary Battlecode: Python bots, Rust engine. Work tracked in [Linear](https://linear.app/andrew-heschl/team/UCBC/overview).

## Setup

Rust 1.85+, Python 3.12, [uv](https://docs.astral.sh/uv/), Node 20.19+, Docker.

```bash
make setup      # env files from the examples, generated code, viewer, engine into .venv
make dev        # after Rust changes: regenerate the SDK and viewer types, rebuild the extension
make test       # rust, python, api (needs Docker), viewer; `make test-api` for the API alone
make lint
make up         # the platform in compose
```

`make` alone lists these. Every target regenerates what it depends on, so the generated
files (`ucbc-sdk/ucbc/games/*/_api.py`, `*.gen.ts`) only ever show up in `git status`,
never as a failure. `GAME=tictactoe` on any target builds that game alone.

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
make up                                 # Postgres, MinIO (:9001 console), API + web on :8000 (/api/docs), worker
uv run ucbc-api-cli create-admin
make down
```

Have a Postgres already? In `.env.local` leave `COMPOSE_PROFILES` empty and set
`POSTGRES_HOST` to it (the example file shows the block); compose then skips its own.

`POST /submissions` uploads a zipped bot (main.py at the top, 1 MiB) to blob storage;
`POST /matches/queue` queues a match between two bot directories on the worker host.

From the checkout instead of compose:

```bash
make db && uv run alembic upgrade head 
uv run ucbc-api                         # API on :8000; `make web` builds the frontend into it
uv run ucbc-worker                      # plays queued matches, UCBC_WORKER_SLOTS at a time
npm run dev -w ucbc-web                 # frontend on :5173, /api proxied
```

`ENV=prod` reads `.env.prod` over `.env.local`. `make deploy DEPLOY_SSH="ssh <host>"` builds
the images here and runs them as prod on that host, which needs Docker and its own
`~/ucbc/.env` and `.env.prod` (see `.env.prod.example`); serve it over HTTPS, the session
cookie is `Secure`. Migrations: `uv run alembic revision -m "..."`,
raw SQL in `op.execute`.

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
| `bots/<game>/` | sample bots | |
| `tests/` | Python integration tests | `make test` |

## Install and release

```bash
pip install ucbc        # Linux x86_64/aarch64 (WSL2 or a container elsewhere): engine, `ucbc`, SDK
pip install ucbc-sdk    # any platform: the SDK alone, for types and completion
```

Bots import `ucbc`; the runner imports `ucbc_engine`. To release, set the same version in
`Cargo.toml`, `ucbc-py/pyproject.toml` (and its `ucbc-sdk==` pin) and
`ucbc-sdk/pyproject.toml`, commit, then `ENV=prod make release` (PyPI, with `PYPI_API_TOKEN` from
`.env.prod`) and `make deploy`, so the platform plays the engine people can install.

Engine internals and adding a game: [docs/engine.md](docs/engine.md). The API, queue, and
worker: [docs/platform.md](docs/platform.md).
