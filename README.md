# UCalgary Battlecode 

UCalgary Battlecode is a 1v1 game between participant implemented python bots.

## Install Package
`pip install ucbc` or `uv add ucbc`

## Use Locally

### Dev Setup

Rust 1.96+, Python 3.12, [uv](https://docs.astral.sh/uv/), Node 20.19+, Docker.

```bash
make setup      # once to setup env files, dependencies, then run `make build`
make build      # after any change to game code, and at setup
make test       # runs all tests. run make test-api to run that alone
make lint
make up         # the full platform in compose. do not run for modular dev testing (runs build)
```
The engine is written to be game agnostic; `default` in `ucbc-games/Cargo.toml` picks the games a build includes.
Tests build all games.

To work on one piece without the full build, see section on [modular execution](#modular-execution).

### Run a match

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


#### Create Maps

`uv run ucbc editor my.map` opens the map editor on a file (new if missing)

### Write a participant bot

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

### Platform API

```bash
make up                                 # Postgres, MinIO (:9001 console), API + web on :8000 (/api/docs), worker
uv run ucbc-api-cli create-admin
make down
```

From the checkout instead of compose:

```bash
make db && uv run alembic upgrade head 
uv run ucbc-api                         # API on :8000; `make web` builds the frontend into it
uv run ucbc-worker                      # queues platform matches, plays queued ones UCBC_WORKER_SLOTS at a time
npm run dev -w ucbc-web                 # frontend on :5173, /api proxied
```

`ENV=prod` reads `.env.prod` over `.env.local`

### Modular execution

Each platform chunk has a dev loop that skips `make build`. 

<details>
<summary>API</summary>

```bash
make db                                 # Postgres and MinIO in Docker (run once if not up)
uv run alembic upgrade head             # Run after database changes (migration files)
uv run ucbc-api                         # API on :8000, docs at /api/docs
uv run ucbc-worker                      # optional: plays queued matches
make test-api                           # tests/api; starts its own Postgres and MinIO containers
```
</details>

<details>
<summary>Web app</summary>

```bash
uv run ucbc-api                         # the dev server proxies /api to :8000 (run once if not up)
npm run dev -w ucbc-web                 # frontend on :5173
npm run typecheck -w ucbc-web
```

Change `UCBC_API_PROXY` env var to proxy to a different api 
</details>

<details>
<summary>Game viewer</summary>

```bash
npm run dev -w @ucbc/viewer             # lists the sample replays in games/<game>/viewer/replays; or drop a replay file on the page
npm test -w @ucbc/viewer
uv run pytest tests/test_viewer.py      # the sample replays against the engine
```

Changed Rust types need `make viewer-types` first
</details>

<details>
<summary>Map editor</summary>

```bash
npm run dev -w @ucbc/editor-ucbc2027    # starts on a new map; Save downloads it
npm test -w @ucbc/editor-ucbc2027
uv run pytest tests/test_editor.py
uv run ucbc editor my.map               # the built page on a file, after `make editor`
```

A changed `map.proto` needs `make editor-types` first.
</details>

### Layout

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
