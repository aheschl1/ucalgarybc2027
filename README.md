# UCalgary Battlecode 

UCalgary Battlecode is a 1v1 game between participant implemented python bots.

## Install
`pip install ucbc` or `uv add ucbc`

## Dev Setup

Rust 1.96+, Python 3.12, [uv](https://docs.astral.sh/uv/), Node 20.19+, Docker.

```bash
make setup      # once: env files, dependencies, then `make build`
make build        # after any change: generated code, the viewer and editor pages, the bot runtime, the engine in .venv
make test       # rust, python, api (needs Docker), viewer; `make test-api` for the API alone
make lint
make up         # the platform in compose
```
The engine is written to be game agnostic; `default` in `ucbc-games/Cargo.toml` picks the games a build includes.

## Run a match

```bash
uv run ucbc run bots/ucbc2027/noop bots/ucbc2027/noop --view
```

| Flag | |
| --- | --- |
| `--seed N`, `--sets N` | |
| `--step-ms`, `--memory-mb` | per bot; default 3 ms, 1 GiB ([limits](docs/resourcelimits.md)) |
| `--show-bot-output` | bot stdout/stderr |
| `--no-verbose` | hide tick progress and time estimates |
| `--replay FILE`, `--summary FILE` | write JSON; a replay named `*.gz` is gzipped |
| `--map FILE` | play on a map file, e.g. `games/ucbc2027/maps/standard.map`; once for every set, or repeated once per set |
| `--view` | open replay in browser |


### Create Maps

`uv run ucbc editor my.map` opens the map editor on a file (new if missing)

### Viewer Dev Setup

To avoid the full build when working on the viewer: `npm ci && npm run dev -w @ucbc/viewer`

## Write a participant bot

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

From the checkout instead of compose:

```bash
make db && uv run alembic upgrade head 
uv run ucbc-api                         # API on :8000; `make web` builds the frontend into it
uv run ucbc-worker                      # plays queued matches, UCBC_WORKER_SLOTS at a time
npm run dev -w ucbc-web                 # frontend on :5173, /api proxied
```

`ENV=prod` reads `.env.prod` over `.env.local`

## Layout

| Path | Builds | Reached as |
| --- | --- | --- |
| `ucbc-engine/` | crate: game-agnostic engine | `use ucbc_engine` |
| `ucbc-games/` | crate: every game, and which a build includes | `ucbc-cli`, `ucbc-dev` |
| `games/<game>/` | crate: one game + its viewer renderer in `viewer/` | listed in `ucbc-games` |
| `ucbc-cli/` | pip dist `ucbc`: what bots import, the engine extension, bot process, CLI | `uv run ucbc`, `import ucbc` |
| `ucbc-dev/` | cargo bin `ucbc-dev`: replays, schemas, SDK generation, the runtime snapshot | `cargo run -p ucbc-dev`, `make sdk` |
| `ucbc-wasm/` | crate: Python bots in wasm; `guest/` is the interpreter they run | `ucbc-cli`, `make runtime`, `make guest` |
| `ucbc-viewer/` | replay viewer (React, Vite) | `ucbc view`, `make viewer` |
| `ucbc-api/` | pip dist `ucbc-api` (FastAPI, Postgres) | `uv run ucbc-api`, `uv run ucbc-api-cli` |
| `ucbc-worker/` | pip dist `ucbc-worker` | `uv run ucbc-worker` |
| `ucbc-web/` | platform frontend (React, Vite) | served by `ucbc-api` at `/`, `make web` |
| `bots/<game>/` | sample bots | |
| `tests/` | Python integration tests | `make test` |
