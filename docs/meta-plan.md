# UCBC 2027 Meta Plan

Approved 2026-09-12. This document is the source of truth for scope, architecture decisions, milestones, and schedule. Requirements live in the [requirements doc](https://docs.google.com/document/d/144TrZ-b5DBa_8d_TAtd_8rvSR4AyLnbrPWWHu0AcfHg/edit). Work is tracked in Linear, mirrored from the structure at the end of this document.

## Context

UCBC (UCalgary Battlecode) is a Battlecode-style PvP bot competition running in Fall 2027. Participants form teams, write Python bots against an engine SDK, and submit them through a web platform. Matches run automatically in the cloud, feed an Elo ladder, and are watchable in a web viewer. The competition runs in four phases, each unlocking mechanics and ending in a sprint tournament. Everything lives in this monorepo and is open-sourced after the competition.

Hard dates:

| Date | Meaning |
| --- | --- |
| 2026-09-14 | Week 1 starts |
| 2027-01-04 | Major milestone: vertical slice demo (code frozen 2026-12-11) |
| 2027-04-30 | All code and testing complete |
| Fall 2027 | Competition runs (1 to 2 months, 4 phases) |

Builders: 2 to 4 part-time students. May to August 2027 is buffer, playtesting, content, and polish. Nothing required for the competition may be scheduled there.

## Decisions (locked)

1. **Engine: Rust.** Library crate `ucbc-engine` with prost-generated protobuf types; no game-specific names in the engine (entities, components, config-driven constants).
2. **Bot runtime: in-process, thread per bot.** The engine embeds CPython via PyO3. Every bot, regardless of language, is the same Rust struct implementing a `Bot` trait (`step(&mut self, ctx) -> StepResult`), owned by the engine's bot registry, executed on its own OS thread. The engine calls the team's entry point with the bot's ID; the team's code branches on ID and type. Per-bot memory is a dict the engine owns and injects on each call. Each bot gets its own module namespace (the team's `main.py` is executed into a fresh globals dict per bot), so module globals are per-bot. Execution is serial; the GIL is never contended.
3. **Engine ships as a Python extension module** (`maturin` wheel, `pip install ucbc`). The same wheel runs matches locally (`ucbc run a/ b/ map`) and in the cloud container. One artifact, no Docker needed for participants.
4. **Time limits: CPU time per step** measured with `CLOCK_THREAD_CPUTIME_ID` on the bot's thread, budget in config. Wall-clock backstop at a multiple of the budget triggers an async exception in the bot thread (`PyThreadState_SetAsyncExc`). If the thread does not unwind within a grace period, the match process aborts and the team forfeits the set. This is the accepted cost of in-process execution and is documented in the rules.
5. **Memory: a per-bot limit, enforced in-process by an allocator hook.** At interpreter start the engine installs a custom allocator through `PyMem_SetAllocator` for all three domains (with pymalloc bypassed so every allocation is visible) and, if numpy is allowed, through numpy's `PyDataMem_SetHandler`. Each allocation carries a small header recording its owner and size; the owner is the bot currently stepping, which the engine sets before each call. Frees are credited to the recorded owner regardless of who frees, so live bytes per bot are exact even though execution is serial. When an allocation would push the current bot past its budget the allocator returns null, Python raises `MemoryError` inside the bot, the step fails, and the bot is removed under the same rule as a non-zero exit; if it is the base, the team loses the set. The container memory limit remains the outer bound for the whole match. Per-bot live bytes are written into the replay for the viewer's stat panel and the admin dashboard. A crash-attribution marker (current bot ID written to a small file before each step) lets the worker blame the right team on abnormal exit. The week 3 spike must show overhead under 20% and correct attribution when bot A allocates and bot B frees; the fallback is a sampled reachability walk from each bot's namespace and memory dict every K ticks.
6. **One engine process per match, one gVisor container per match.** No Kubernetes. Postgres-backed job queue; a worker on one VM launches containers. Parallelism = more workers.
7. **Four phases.** P0 movement, terrain, pathfinding, mining. P1 economy, banks, leveling/merging. P2 attack, defense, walls. P3 restricted local comms plus one environmental hazard. Comms hijacking is removed from the game entirely. Natural-disaster scheduling is deferred past April.
8. **Maps as text grids** compiled to protobuf; the text format stays the source of truth. A map editor ships by M5 (April 30): edit terrain, resources, and spawn points on the grid, import and export the text format, validate against the map schema. Work starts after M3 so it reuses the viewer's renderer.
9. **Viewer: stack chosen by the viewer owner** by the end of week 2. This plan fixes only what it must do: play a replay file at variable speed, step and seek one state at a time, show per-bot stdout with a toggle, show resources and key game variables, run on a weak laptop and a phone, embed in the platform's match page, and run standalone against a local replay file. Framework, rendering approach, and asset pipeline are the owner's call, recorded in a short decision note in `docs/`.
10. **Platform backend: Python, FastAPI, Pydantic**, Postgres via SQLAlchemy 2.0 with Alembic migrations (SQLModel is acceptable if the owner prefers), Discord OAuth. The match worker is Python in the same package and shares the Pydantic models and queue tables. The API publishes an OpenAPI schema; the participant CLI (Python, shipped in the `ucbc` pip package) and the web frontend are generated or typed clients of it. Deployed with docker compose behind Caddy on the same VM as the workers. Replays in S3-compatible object storage.
11. **Platform frontend: a separate web app** consuming the FastAPI API. Stack chosen by the frontend owner by the end of week 2, ideally the same stack as the viewer so one person can own both. Requirements only: sign-in, team pages, submissions, match pages embedding the viewer, ladder, docs pages, admin console, usable on a phone.
12. **Sandbox is a first-class requirement:** gVisor runtime from the first image, no network, read-only filesystem, non-root, pids and memory limits. A Python import allowlist (stdlib subset plus numpy) exists for fairness and crash prevention, not security; the container is the security boundary.
13. **Reference bots are a deliverable:** one per phase, used for engine tests, balance, the tutorial, and ladder seeding.
14. **Replay format decided by measurement** in week 5: full state per tick with zstd if a 1000-tick, 100-entity replay is under 10 MB gzipped, otherwise keyframes every K ticks plus deltas.
15. **"Users cannot access raw state" is reframed:** replays are public data; bot source is the secret.

## Architecture

```text
web frontend ──▶ platform API (FastAPI) ──enqueue──▶ Postgres queue ◀──poll── worker (Python, same package)
     │                    ▲                                                     │ docker run --runtime=runsc
     │ embeds             │ participant CLI (Python)                            ▼
  viewer          ◀──replay.pb from object storage◀────────────── match container: `ucbc run`
                                                        └─ Python host process
                                                           └─ ucbc-engine (Rust, PyO3)
                                                              ├─ Bot registry: Vec<BotHandle>
                                                              │   each = thread + namespace + memory
                                                              ├─ tick loop, systems, win check
                                                              └─ replay writer + stdout capture
```

Repo layout (monorepo):

```text
proto/        buf.yaml, map.proto, game_state.proto, replay.proto, job.proto   (owned by engine)
engine/       cargo workspace: ucbc-engine (lib), ucbc-py (PyO3 ext), ucbc-cli
sdk/          python package `ucbc`: Api, runner, participant CLI, import hook
viewer/       game viewer (stack per viewer owner), standalone local page
platform/     api/ (FastAPI, Pydantic, SQLAlchemy, Alembic), worker/ (Python, shares models), web/ (frontend app)
infra/        Dockerfile.match (gVisor), compose, Caddy, deploy scripts
bots/         reference bots: random_walker, greedy_miner, phase0..phase3
maps/         text maps + compiled .pb
docs/         rules, api reference, tutorial, meta-plan.md
```

The engine sees every bot through one Rust type and trait, so a future year could add a `WasmBot` or `RustBot` without touching the tick loop. Python is the only implementation for 2027.

## Workstreams (Linear projects)

| # | Project | Owner | Done at |
| --- | --- | --- | --- |
| 1 | Game Design & Rules | TBD | M4 |
| 2 | Schemas & Contracts | engine owner | M2 |
| 3 | Engine | TBD | M4 |
| 4 | Bot Runtime & SDK | TBD | M4 |
| 5 | Game Viewer | TBD | M5 |
| 6 | Platform Backend | TBD | M4 |
| 7 | Platform Frontend | TBD | M4 |
| 8 | Infrastructure & Ops | TBD | M4 |
| 9 | Reference Bots & Participant CLI | TBD | M4 |
| 10 | Competition Operations | Andrew | M5 |

With 2 to 4 people, expect one person to own 2 or 3 projects. Engine + Runtime + Schemas should share an owner. Viewer + Frontend can share. Backend + Infra can share.

## Milestones

| Milestone | Date | Definition of done |
| --- | --- | --- |
| M0 Foundations | 2026-10-02 | Rules v1 frozen with numeric constants; schemas v0; monorepo scaffold with CI for Rust/Python/TS; cloud account, domain, one VM, object storage; runtime spike go/no-go; owners assigned. |
| M1 Local Match | 2026-10-30 | `ucbc run` plays a full match locally with movement, terrain, mining, spawn, attack, win check; per-bot CPU and memory limits enforced; random-walker and greedy-miner bots; replay written; viewer plays it with speed and step controls. |
| M2 Vertical Slice | code freeze 2026-12-11, demo 2027-01-04 | A fresh user signs in with Discord, forms a team, submits a bot via web or CLI; a job runs in a gVisor container in the cloud; the replay lands in object storage; the match page plays it with per-bot stdout. Bugfix-only over the break. |
| M3 Feature Complete | 2027-02-26 | All four phases' mechanics in the engine; phase gating configs; pinning, unranked and self-test matches; ranked wave scheduler with Swiss pairing; Elo and ladder; public/private match bins; sprint bracket generator; admin bans, logs, manual wave trigger; one reference bot per phase; docs skeleton. |
| M4 Hardened | 2027-04-02 | Load test at 50 teams per wave; security review with sandbox escape attempts; balance pass with reference bots; viewer verified on a weak laptop and a real phone; rules, API reference, tutorial complete; monitoring, alerts, backups; simple admin dashboard with performance metrics, engine errors, and submission trends. |
| M5 Complete | 2027-04-30 | Map editor shipped; closed alpha with 3 to 5 friendly teams finished and their bugs fixed; bug bash; golden replay tests green; retro; freeze. |

Critical path to M2: rules v0 → schemas v0 → runtime spike → tick loop + Api → reference bot → replay writer → container image + worker + storage → submission endpoint → match page. Platform auth and viewer rendering run in parallel off a hand-written replay fixture.

Critical path to M5: rules v1 → all mechanics → reference bot per phase → balance → phase configs frozen → docs. This slips on design, not code, so rules v1 has a hard freeze at M0 and the engine owner has veto on changes after it.

## Schedule, first eight weeks

| Week | Engine / Runtime / Schemas | Viewer / Frontend | Backend / Infra | Design / Ops |
| --- | --- | --- | --- | --- |
| W1 Sep 14 | Cargo workspace, buf codegen, CI | Viewer and frontend stack decision notes | Cloud account, domain, VM, bucket; FastAPI scaffold | Owners; rules v0; org ownership decision |
| W2 Sep 21 | map/game_state/replay proto v0; config loader; map parser; empty tick loop | Viewer scaffold; renders a map from a fixture | Postgres, SQLAlchemy models, Alembic baseline; OpenAPI published | Rules v1 draft with constants |
| W3 Sep 28 | Runtime spike: PyO3 embed, thread per bot, per-bot namespace, async-exception timeout, CPU metering, per-bot memory accounting allocator; 128 bots × 1000 ticks | Fixture-driven playback loop | Discord OAuth | Rules v1 frozen; entity cap, CPU budget, and memory budget set from spike |
| W4 Oct 5 | Movement, terrain, vision; Bot trait + registry; Api v0 | Speed/step/seek controls | Match container image with gVisor | Economy and leveling design |
| W5 Oct 12 | Mining, spawn, attack, walls, win check; replay writer; replay size measurement | Plays a real replay | Job queue table + worker runs a container | Combat and defense design |
| W6 Oct 19 | Import allowlist; per-bot memory; stdout capture | Stdout and stat panels | Cloud end to end: row → container → replay in bucket | Discord server |
| W7 Oct 26 | `pip install ucbc` wheel, `ucbc run` | Embeddable API + local drop page | Submission endpoint, storage, validation; CLI submit | Greedy reference bot |
| W8 Nov 2 | Timeout/forfeit semantics; crash attribution | Match page embedding viewer | Teams: create/join/invite | Slice dry run from a fresh account; January cut list |

W9 to W13 (Nov 9 to Dec 11): finish M2 scope, integration weeks, bug bash, freeze. W14 to W16: break, bugfix only. W17 (Jan 4): demo and M2 declared.

## Removed from the game

- Comms hijacking. Not deferred; it is out.

## Scope explicitly deferred to May–August 2027

- Natural disasters, time bank
- Story beyond a one-page premise and phase names
- Art and animation polish beyond what the viewer needs to be readable
- Admin dashboard beyond the simple M4 version (drill-downs, per-team history, alerting rules)
- Autoscaling; fixed VM pool with queue depth on a status page instead
- Touch-optimized phone UI (phone must work, not be optimized)
- Announcements CMS; a markdown file is enough

## Risks

| Risk | Mitigation | By |
| --- | --- | --- |
| Rules never freeze, engine blocked | Rules v1 with numbers frozen; engine owner veto after | W3 |
| In-process runtime cannot time out or isolate bots | Week 3 spike proves async-exception unwind and per-bot namespaces; fallback is one Python subinterpreter per bot (3.14 `concurrent.interpreters`), then process-per-team as last resort | W3 |
| Per-bot memory accounting too slow or misattributes | Same spike measures allocator-hook overhead and runs an allocate-in-A, free-in-B test; fallback is a sampled reachability walk per bot every K ticks with the container limit as the hard bound | W3 |
| January lands after finals | Internal freeze Dec 11; demo is a recorded run | W13 |
| Cloud path built last | Row → container → replay in bucket working before any platform UI | W6 |
| Replays too big for phones | Measure a synthetic 1000-tick replay; switch to keyframes plus deltas if over 10 MB | W5 |
| A team crashes the shared match process | Crash attribution marker; forfeit rule; import allowlist blocks ctypes, os, subprocess, socket, threading, multiprocessing | W8 |
| 2 to 4 people, 10 workstreams | Owners hold multiple projects; scope cuts above are firm; every milestone has a cut list review one week before | ongoing |

## Linear structure

**Team:** one team, `UCBC`.

**Projects:** the ten workstreams above, each with a lead, a description linking the requirements doc and this document, and a target date at its "Done at" milestone.

**Milestones:** M0 to M5 as project milestones in every project they apply to, with the dates above.

**Labels:** `engine`, `sdk`, `viewer`, `platform`, `infra`, `design`, `bots`, `ops`, `docs`, `spike`, `phase-0`, `phase-1`, `phase-2`, `phase-3`.

**Initial issues** (tracking issue per project holds the checklist; the rest are the first concrete tasks, tagged with milestone):

1. Game Design & Rules: tracking; rules v0 slice ruleset (M0); four phase gating configs (M0); entity cap, tick count, CPU budget, and per-bot memory budget from spike (M0); economy and leveling design with numbers (M1); combat, defense, walls design (M1); restricted comms and hazard design (M3); balance pass per phase (M4); one-page story premise and phase names (M5).
2. Schemas & Contracts: tracking; map.proto v0 plus text map format and compiler (M0); game_state.proto v0 (M0); replay.proto v0 and size measurement (M1); Python Api surface spec (M1); platform-to-worker job contract (M2).
3. Engine: tracking; workspace, buf codegen, CI (M0); config loader, map parser, tick loop (M0); Bot trait, registry, spawn/despawn (M0); movement, terrain, collision, vision (M1); mining, resources, spawning (M1); attack, damage, walls, win conditions (M1); replay writer and stdout capture (M1); economy and leveling/merge (M3); restricted comms and hazard system (M3); seeded RNG and replay hash test (M3); golden replay test suite (M4).
4. Bot Runtime & SDK: tracking; runtime spike go/no-go covering timeout, isolation, and memory accounting (M0, `spike`); pinned Python version and numpy policy (M0); per-bot namespace and injected memory (M1); per-bot memory accounting allocator with MemoryError enforcement and numpy handler (M1); Api object: observations, actions, memory, logging (M1); import allowlist hook (M1); timeout, forfeit, crash attribution (M2); maturin wheel and `ucbc run` (M2); SDK reference docs generation (M4).
5. Game Viewer: tracking; stack decision note (M0); scaffold, replay decoder, map rendered from a fixture (M0); playback at variable speed, step, seek (M1); per-bot stdout panel with toggle and game-variable panel (M2); embed in match page and standalone local replay page (M2); first pass at assets (M3); weak laptop and phone performance pass (M4); map editor on the viewer's renderer with text-format import/export and schema validation (M5).
6. Platform Backend: tracking; FastAPI, Pydantic, SQLAlchemy, Alembic scaffold with OpenAPI published (M0); Discord OAuth and sessions (M1); teams (M2); submissions with versioning and validation (M2); job queue and worker (M2); match model with public/private bins (M2); pinning, unranked, self-test matches (M3); ranked wave scheduler, Elo, ladder (M3); sprint snapshot and bracket (M3); admin bans, logs, manual wave (M3); admin metrics endpoints: match throughput, queue depth, engine error rate, submissions per day (M4); rate limits and abuse controls (M4).
7. Platform Frontend: tracking; stack decision note and typed API client from OpenAPI (M0); site shell, auth, team page (M2); match page with embedded viewer (M2); ladder with near-live updates (M3); rules, API reference, tutorial, announcements pages (M4); admin console UI for bans, logs, manual wave (M3); simple admin dashboard: performance metrics, engine errors, submission trends (M4); mobile pass (M4).
8. Infrastructure & Ops: tracking; cloud account, domain, DNS, VM, bucket (M0); match image with gVisor and limits (M1); compose deploy, Caddy, CI deploy (M2); monitoring, alerts, backups (M3); load test 50 teams per wave (M4); security review (M4).
9. Reference Bots & Participant CLI: tracking; random walker and greedy miner (M1); CLI in the `ucbc` package using the generated API client: login, submit, pin, request match, results (M2); reference bot per phase (M3); minimal-bot tutorial (M4).
10. Competition Operations: tracking; org ownership and registration (M0); Discord server and comms calendar (M2); prizes and sponsorship (M3); closed alpha with friendly teams (M5); open-source release checklist and license (M5).
