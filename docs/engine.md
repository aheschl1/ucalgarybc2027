# Engine

## Crates

```mermaid
graph LR
    engine[ucbc-engine<br/>game-agnostic core]
    ttt[games/tictactoe<br/>impl Game]
    g27[games/ucbc2027<br/>impl Game]
    games[ucbc-games<br/>every game, feature per game]
    py[ucbc-cli<br/>python package ucbc: ucbc._engine, bot process, handles]
    cli[ucbc-dev]
    ttt --> engine
    games -. feature per game .-> ttt
    games -. feature per game .-> g27
    g27 --> engine
    py --> engine
    py -- default features --> games
    cli --> engine
    cli -- all --> games
```

## Vocabulary

```text
match  = sets            (first team rotates each set)
set    = ticks           (game.status() Complete, or max_ticks -> game.tick_limit())
tick   = one run of game.schedule()
step   = one bot's turn within a tick
team   = one code submission; owns many bots
bot    = one unit the game asks to step; own runtime, own namespace
```

## Match loop

```mermaid
flowchart TD
    A[for set in 0..sets] --> B[game = Game::create]
    B --> C{status Complete<br/>or tick == max_ticks?}
    C -- yes --> Z[SetResult<br/>at max_ticks: game.tick_limit]
    C -- no --> D[schedule = game.schedule]
    D --> E[for bot in schedule, skip if removed]
    E --> G[registry.bot_mut: spawn on first use]
    G --> H[bot.step&#40;StepCtx&#41;]
    H -- Ok --> I[game.end_step]
    H -- Err --> J[game.bot_failed<br/>remove from game.bots]
    I --> K[drop runtimes of bots<br/>no longer in game.bots]
    J --> K
    K --> L[record Step]
    L --> E
    E -- done --> M[game.end_tick unless Complete]
    M --> N[record Tick + snapshot]
    N --> C
```

## Game trait

```rust
pub trait Game: Send + 'static {
    const NAME: &'static str;
    type Query: DeserializeOwned + JsonSchema;       // #[serde(tag = "type")] enum of Request<P, R>
    type Action: DeserializeOwned + JsonSchema;      // #[serde(tag = "type")] enum of Request<P, R>
    type Snapshot: Serialize + JsonSchema;
    type Bot;                                        // per-bot data in the BotManager

    fn create(setup: &SetSetup) -> Result<Self, EngineError>;
    fn bots(&self) -> &BotManager<Self::Bot>;
    fn bots_mut(&mut self) -> &mut BotManager<Self::Bot>;
    fn schedule(&mut self) -> Vec<BotId> { self.bots().ids() }
    fn handle_query(&self, bot: BotId, query: Self::Query) -> Result<Answer, QueryError>;
    fn apply_action(&mut self, bot: BotId, action: Self::Action) -> Result<Answer, ActionError>;
    fn end_step(&mut self, bot: BotId);
    fn bot_failed(&mut self, bot: BotId, failure: &BotFailure) {}
    fn end_tick(&mut self);
    fn status(&self) -> GameStatus;
    fn tick_limit(&self, max_ticks: u32) -> Outcome { Outcome::draw(..) }
    fn snapshot(&self) -> Self::Snapshot;
}
```

Games never see JSON. `DynGame` (blanket impl) decodes and encodes at the boundary.

A game keeps its bots in a `BotManager<T>`: `spawn(team, data)` hands out the next id
(never reused in a set) and the bot steps from the next tick; `remove(id)` takes it
out; `manager[id]` is a `BotWrap<T>` that derefs to `T` and has `team()`. The default
schedule is spawn order. After a failure the engine calls `bot_failed`, then removes the
bot itself.

Each query and action variant holds a `Request<P, R>`: parameters `P` (derefs to them),
reply `R`. `reply(r)` is the only way to build an `Answer`, so each is answered with its
own type, and `R` is the method's return type in the generated Python.

```rust
enum Queries {
    /// What stands on a tile.
    Item(Request<At, Option<Item>>),
    Width(Request<(), usize>),
}

enum Action {
    Place(Request<Spot, Placed>),
    Noop(Request<(), ()>),
}
```

```mermaid
classDiagram
    class Game { <<trait, typed>> }
    class DynGame { <<trait, serde_json::Value>> }
    class TicTacToe
    class GameRegistry { register~G: Game~() ; get(name) }
    TicTacToe ..|> Game
    Game ..|> DynGame : blanket impl
    GameRegistry o-- DynGame : Box<dyn DynGame> factories
```

Registration:

```rust
let mut registry = GameRegistry::new();
registry.register::<TicTacToe>();
```

## Bot trait

```rust
pub trait Bot: Send {
    fn step(&mut self, ctx: &mut StepCtx<'_>) -> StepResult;
    fn shutdown(&mut self) {}
}

pub type BotFactory = Box<dyn FnMut(&SpawnCtx) -> Result<Box<dyn Bot>, BotFailure> + Send>;

TeamSpec::new(name, factory)
TeamSpec::rust(name, |ctx: &SpawnCtx| MyBot)
TeamSpec::unavailable(name, failure)   // every bot fails at its first step
```

Bot runtimes are created lazily when the game first schedules them, and released once
the bot leaves the game's `BotManager`, or at set end.

## StepCtx: the bot's only view of the game

```rust
pub struct StepCtx<'a> {
    pub bot: BotId,
    pub team: &'a TeamInfo,
    pub set_index: u32,
    pub tick: u32,
    pub seed: u64,
    // private: &mut dyn DynGame, accepted actions
}

ctx.query(&json!({"type": "board"}))?;              // read-only
ctx.act(&json!({"type": "place", "row": 0, "col": 2}))?;  // Err = rejected, state unchanged
ctx.status();
```

```text
ActionError::Invalid / UnknownType / Malformed  -> bot may try again
ActionError::SetOver                             -> set already complete
BotFailure::Exception / Crash                    -> engine drops the bot, game.bot_failed decides
```

## Python bots

One wasm instance per bot (`ucbc-cli/src/bot.rs` over `ucbc-wasm`): CPython for WASI,
started from a snapshot, run on fuel, pickled messages through one host call. See
[wasm.md](wasm.md) and [resourcelimits.md](resourcelimits.md).

```mermaid
sequenceDiagram
    participant R as runner
    participant B as bot (wasm)
    R->>B: load
    B->>R: ready
    R-->>B: {identity}
    B->>R: loaded (failure | null)
    loop each turn
        R->>B: step(set_index, tick)
        B->>R: query | act
        R-->>B: reply {ok} | {err}
        B->>R: done {stdout, error}
    end
```

Inside the instance: `ucbc._guest`, already imported in the snapshot, imports `main.py`
from `/bot` and buffers the bot's prints. `ucbc.handle.Handle` wraps the host call as
`_query(dict)` and `_act(dict)`; `ucbc.games.<g>.HANDLE` is the typed subclass a bot's
`step` receives.

Reply format:

```json
{"ok": {...}}
{"err": {"kind": "query" | "action" | "set_over", "message": "..."}}
```

## Replay

```text
Replay { match_id, engine_version, config { game, sets, seed, teams, max_ticks, limits, game_config? }, teams, sets[], result }
  SetReplay { index, first_team, initial_state, ticks[], result }
    Tick { number, steps[], state_after }
      Step { bot, team, actions[], stdout, failure?, usage: { time_us, memory? } }
    SetResult { index, first_team, winner_team?, reason: win|draw|forfeit, detail, ticks }
  MatchResult { sets[], set_wins[], winner_team? }
```

Deterministic for a fixed seed, `usage` included: no timestamps, sorted JSON keys, ChaCha8
seeds (`set_seed(match_seed, set)`, `bot_seed(set_seed, bot)`).

## Games

```text
ucbc2027    the competition game and the default build. Labs spawn dinos for bones; dinos
            move and carry fossils home to raise income. No win condition yet, so every
            set draws at the tick limit.
tictactoe   the example game, and what the bot-runtime tests play.
```

## Adding a game

```text
games/foo/src/game.rs                       crate ucbc-foo: impl Game for Foo { const NAME = "foo"; ... }
                                            every query, action, reply, and snapshot type derives JsonSchema
games/foo/viewer/                           package @ucbc/viewer-foo exporting `renderer`
ucbc-games/Cargo.toml                       foo = ["dep:ucbc-foo"], its path dependency, foo in `all`,
                                            and foo in `default` for builds to include it
ucbc-games/src/lib.rs                       #[cfg(feature = "foo")] registry.register::<ucbc_foo::Foo>();
ucbc-cli/python/ucbc/games/foo/_api.py      generated by `make sdk` (ucbc-dev gen-sdk): FooApi(Handle)
                                            with one method per query and action, one class per reply type
ucbc-cli/python/ucbc/games/foo/__init__.py  class FooHandle(FooApi): conveniences; HANDLE = FooHandle
bots/foo/<name>/main.py                     def step(handle: FooHandle) -> None: ...
```

The folder is named for the game: the viewer build, `make viewer-types`, and the Docker
images find it under `games/`, and `npm install` links its renderer.

`ucbc-dev api foo` prints the schemas the generator reads. Both generated files are
committed; `make gen` (run by `dev`, `lint`, `test`, `viewer`, `web`) rewrites them, so a
stale one shows up in `git status` after any of those.

```bash
make dev && uv run ucbc run bots/foo/a bots/foo/b
```
