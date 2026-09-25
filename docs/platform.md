# Platform

```mermaid
graph LR
    web[web app<br/>served at /]
    api[ucbc-api<br/>FastAPI at /api]
    db[(Postgres<br/>matches = queue)]
    blobs[(blob storage<br/>S3 API, MinIO locally)]
    worker[ucbc-worker]
    engine[ucbc run<br/>one process per match]
    web --> api
    api --> db
    api --> blobs
    worker --> db
    worker --> blobs
    worker --> engine
```

Two images from root Dockerfiles: `Dockerfile.api` (API plus the built web app) and
`Dockerfile.worker` (API package plus the engine wheel). `compose.yaml` runs `minio`, `api`,
`worker`, and `db` when `COMPOSE_PROFILES=db` in `.env.<ENV>`; with it empty, `POSTGRES_HOST`
(and `POSTGRES_PORT`, `POSTGRES_DB`) name an existing Postgres instead. To reach one on
another Docker network, a git-ignored `compose.override.yaml` can attach `api` and `worker`
to that network and `POSTGRES_HOST` its container name.

## Auth

`POST /api/auth/login` takes an email and password, checks the password (Argon2), and sets a
session cookie:
`__Host-ucbc_session`, `HttpOnly`, `Secure`, `SameSite=Strict`, 30 days. The cookie holds a
random token; `sessions` holds its SHA-256 and the user id, so a database read yields no live
session. Every request resolves the cookie against `sessions` joined to `users`, so deleting a
user or a session row ends access at once. `POST /api/auth/logout` deletes the row. The web
app stores nothing itself; on load it asks `/api/users/me` whether it is logged in.
`SameSite=Strict` is the CSRF defence; it works because every API call is a same-site `fetch`.
Login is limited to 60 requests a minute per client address (slowapi, in memory); behind a
proxy, uvicorn must be told to trust the forwarded address for that to be per client.

## Users

A user is `email`, `display_name`, `password_hash`, `is_admin`. The email is the login
credential and is never shown to anyone but its owner; the display name is the only thing
another user sees, next to a submission. `POST /api/users` is open: anyone signs up, always
as a non-admin, and the app logs in straight after. Admins are made only by
`ucbc-api-cli create-admin`. Nothing about a user can be changed once it exists.

The web app is a single page with client-side routes. `api/api.py::APP_ROUTES` lists the
paths it owns, each served the app shell so a reload of one works; anything else under `/`
is a built file or a 404. Signed out, every path is the login form except `/register` and
`/docs`, the participant guide, which renders `ucbc-web/src/docs.md`.

## Teams

A participant team (not the engine's in-game teams) owns submissions. Every user is on
exactly one (`users.team_id`); signup creates one named after the display name. Team names
are unique ignoring case, so a clashing display name gets the first free number
(`Alice 2`).

`/api/teams` acts on the caller's team only. Every member sees its join code and may replace
it. Anyone with the code joins; creating a team or joining one moves the caller out of the
last, which keeps its submissions and its name. Members are equal and there is no size cap.

## Submissions

A submission is a zip with `main.py` at the top, at most 1 MiB, checked on upload for member
paths that escape the directory. The row (`submissions`: uploader, the uploader's team, name,
game, size, sha256) is in Postgres; the zip is in the bucket at `submissions/<id>.zip`
(`api/blobs.py`, one boto3 client behind `UCBC_BLOB_URL`). Metadata is visible to every logged-in user, the code is not; `?mine=true` narrows the
list to the caller's team's.

## Queue

A match row is the queue entry.

```mermaid
stateDiagram-v2
    [*] --> queued: POST /api/matches/queue
    queued --> running: claim
    running --> done: replay recorded
    running --> error: ucbc run failed, or too many attempts
    running --> queued: engine could not start
    running --> running: heartbeat stale, claimed again
```

A worker claims with `for update skip locked`, heartbeats while the engine runs, and writes
the sets and result in one transaction. `(id, claimed_at)` is the lease: a stale heartbeat
lets another worker take the match, after which the old holder's writes match no row.

`matches.bots` names each bot's code: `{kind: "path", path}` (admins only) or
`{kind: "submission", id}`, which the worker downloads and unpacks (`worker/match.py::fetch`).
`worker/match.py::command` is the one place the engine process is described.

Who sees a match: admins see all; everyone sees `origin = 'platform'` matches (from schedules,
not built yet); a user sees matches with one of their own submissions. `?mine=true` narrows
any caller to the matches one of their own submissions is in. A member may queue a
match between submissions when one is their own.

`/viewer/<match id>` in the web app plays a done match: it reads the match and each of its
sets through these endpoints and hands the assembled replay to the viewer (`ucbc-viewer`), so
it shows exactly the matches the API shows the caller. `make web GAME=...` picks the game
renderer it bundles (`ucbc-viewer/vite/games.ts`).

## Settings

`UCBC_` variables from `.env`, then `.env.local` or `.env.prod`: `DATABASE_URL`,
`BLOB_URL` (`scheme://access:secret@host[:port]/bucket[?region=]`), `API_HOST`,
`API_PORT`, `ADMIN_EMAIL`, `ADMIN_PASSWORD`, `WORKER_SLOTS`, `WORKER_POLL_S`, `WORKER_HEARTBEAT_S`, `WORKER_LEASE_S`,
`WORKER_NAME`.
