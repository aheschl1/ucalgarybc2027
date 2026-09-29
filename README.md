# UCBC

UCalgary Battlecode: Python bots, Rust engine. Work tracked in [Linear](https://linear.app/andrew-heschl/team/UCBC/overview).

## Setup

Rust 1.96+, Python 3.12, [uv](https://docs.astral.sh/uv/), Node 20.19+, Docker.

```bash
make setup      # once: env files, dependencies, then `make dev`
make dev        # after any change: generated code, the viewer page, the bot runtime, the engine in .venv
make test       # rust, python, api (needs Docker), viewer; `make test-api` for the API alone
make lint
make up         # the platform in compose
```

`default` in `ucbc-games/Cargo.toml` picks the games a build includes; `make test` builds
every game.

## Run a match

```bash
uv run ucbc run bots/ucbc2027/noop bots/ucbc2027/noop --view
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
# bots/ucbc2027/mine/main.py
from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    handle.noop()
```

```bash
uv run ucbc run bots/ucbc2027/mine bots/ucbc2027/noop
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
`POST /matches/queue` queues a match between two submissions.

From the checkout instead of compose:

```bash
make db && uv run alembic upgrade head 
uv run ucbc-api                         # API on :8000; `make web` builds the frontend into it
uv run ucbc-worker                      # plays queued matches, UCBC_WORKER_SLOTS at a time
npm run dev -w ucbc-web                 # frontend on :5173, /api proxied
```

`ENV=prod` reads `.env.prod` over `.env.local` (see `.env.prod.example`); serve it over
HTTPS, the session cookie is `Secure`. Migrations: `uv run alembic revision -m "..."`,
raw SQL in `op.execute`.

## Layout

| Path | Builds | Reached as |
| --- | --- | --- |
| `ucbc-engine/` | crate: game-agnostic engine | `use ucbc_engine` |
| `ucbc-games/` | crate: every game, and which a build includes | `ucbc-cli`, `ucbc-dev` |
| `games/<game>/` | crate: one game + its viewer renderer in `viewer/` | listed in `ucbc-games` |
| `ucbc-cli/` | pip dist `ucbc`: what bots import, the engine extension, bot process, CLI | `uv run ucbc`, `import ucbc` |
| `ucbc-dev/` | cargo bin `ucbc-dev`: replays, schemas, SDK generation, the runtime snapshot | `cargo run -p ucbc-dev`, `make sdk` |
| `ucbc-wasm/` | crate: Python bots in wasm; `guest/` is the interpreter they run | `ucbc-cli`, `make runtime`, `make guest` |
| `ucbc-viewer/` | replay viewer (TS, Vite) | `ucbc view`, `make viewer` |
| `ucbc-api/` | pip dist `ucbc-api` (FastAPI, Postgres) | `uv run ucbc-api`, `uv run ucbc-api-cli` |
| `ucbc-worker/` | pip dist `ucbc-worker` | `uv run ucbc-worker` |
| `ucbc-web/` | platform frontend (React, Vite) | served by `ucbc-api` at `/`, `make web` |
| `bots/<game>/` | sample bots | |
| `tests/` | Python integration tests | `make test` |

## Install and release

```bash
pip install ucbc        # Linux x86_64/aarch64 (WSL2 or a container elsewhere)
```

To release, set the same version in `Cargo.toml` and `ucbc-cli/pyproject.toml`, commit, then `ENV=prod make release` (PyPI, with `PYPI_API_TOKEN` from
`.env.prod`).

Engine internals and adding a game: [docs/engine.md](docs/engine.md). The bot runtime:
[docs/wasm.md](docs/wasm.md). The API, queue, and worker: [docs/platform.md](docs/platform.md).
