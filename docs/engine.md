# Engine

## Crates

```mermaid
graph LR
    engine[ucbc-engine<br/>game-agnostic core]
    ttt[ucbc-tictactoe<br/>impl Game]
    py[ucbc-py<br/>ucbc_engine._engine + bot process]
    cli[ucbc-dev]
    sdk[ucbc-sdk<br/>python: handle, games/]
    ttt --> engine
    py --> engine
    py -. feature per game .-> ttt
    cli --> engine
    cli --> ttt
    py -. imported by the bot process .-> sdk
```

## Vocabulary

```text
match  = sets            (first team rotates each set)
set    = ticks           (game.status() Complete, or max_ticks -> draw)
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
    C -- yes --> Z[SetResult]
    C -- no --> D[schedule = game.schedule]
    D --> E[for bot in schedule]
    E --> G[registry.bot_mut: spawn on first use]
    G --> H[bot.step&#40;StepCtx&#41;]
    H -- Ok --> I[game.end_step]
    H -- Err --> J[registry.despawn<br/>game.bot_failed]
    I --> K[drain game.despawned]
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
    type Query: DeserializeOwned + JsonSchema;       // #[serde(tag = "type")] enum
    type QueryResponse: Serialize + JsonSchema;
    type Action: DeserializeOwned + JsonSchema;      // #[serde(tag = "type")] enum
    type ActionResponse: Serialize + JsonSchema;
    type Snapshot: Serialize + JsonSchema;

    fn create(setup: &SetSetup) -> Result<Self, EngineError>;
    fn schedule(&mut self) -> Vec<BotRef>;
    fn handle_query(&self, bot: BotRef, query: Self::Query) -> Result<Self::QueryResponse, QueryError>;
    fn apply_action(&mut self, bot: BotRef, action: Self::Action) -> Result<Self::ActionResponse, ActionError>;
    fn end_step(&mut self, bot: BotRef);
    fn bot_failed(&mut self, bot: BotRef, failure: &BotFailure);
    fn end_tick(&mut self);
    fn despawned(&mut self) -> Vec<BotId> { Vec::new() }
    fn status(&self) -> GameStatus;
    fn snapshot(&self) -> Self::Snapshot;
}
```

Games never see JSON. `DynGame` (blanket impl) decodes and encodes at the boundary.

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

Bots are created lazily when the game first schedules them, and released on
`despawned()` or at set end. A released id stays dead for the set.

## StepCtx: the bot's only view of the game

```rust
pub struct StepCtx<'a> {
    pub bot: BotRef,
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

One process per bot: `python -m ucbc_engine._bot` (`ucbc-py/src/process.rs`,
`ucbc_engine/_bot.py`), line-delimited JSON over its stdin and stdout. The process
is the unit of isolation: `RLIMIT_AS` for memory, `SIGSTOP` at the step deadline,
`SIGKILL` on despawn. See [resourcelimits.md](resourcelimits.md).

```mermaid
sequenceDiagram
    participant R as runner
    participant B as bot process
    R->>B: init {identity, source, path, memory_bytes}
    B->>B: import ucbc + PRELOAD, setrlimit, _engine.lockdown()
    B-->>R: ready
    B-->>R: loaded (failure | null)
    loop each turn
        R->>B: step {set_index, tick}
        B->>R: query | act
        R-->>B: reply {ok} | {err}
        B-->>R: done {stdout, error, memory}
    end
```

Inside the process: `_bot.py` takes the real stdin/stdout for the link before
`main.py` runs, buffers the bot's prints, and after `lockdown()` the process can open
no files, so a bot may import only `ucbc` and the stdlib modules in `PRELOAD`.
`ucbc.handle.Handle` wraps the link as `_query(dict)` and `_act(dict)`;
`ucbc.games.<g>.HANDLE` is the typed subclass a bot's `step` receives.

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

Deterministic for a fixed seed except `usage`: no timestamps, sorted JSON keys, ChaCha8
seeds (`set_seed(match_seed, set)`, `bot_seed(set_seed, bot)`).

## Adding a game

```text
ucbc-foo/src/game.rs                  impl Game for Foo { const NAME = "foo"; ... }
                                      every Query/Action/Response/Snapshot type derives JsonSchema
ucbc-py/Cargo.toml                    foo = ["dep:ucbc-foo"]     (+ in default)
ucbc-py/src/lib.rs                    #[cfg(feature = "foo")] registry.register::<ucbc_foo::Foo>();
ucbc-dev/src/main.rs                  registry.register::<Foo>();
ucbc-sdk/ucbc/games/foo/_api.py       generated by `make sdk` (ucbc-dev gen-sdk): FooApi(Handle) with
                                      one method per query and action, one class per response type
ucbc-sdk/ucbc/games/foo/__init__.py   class FooHandle(FooApi): conveniences; HANDLE = FooHandle
bots/foo/<name>/main.py               def step(handle: FooHandle) -> None: ...
ucbc-foo/viewer/                      package @ucbc/viewer-foo exporting `renderer`: a workspace in package.json
                                      and a dependency of ucbc-viewer, which is how builds and `make viewer-types`
                                      find it
```

`ucbc-dev api foo` prints the schemas the generator reads. `make lint` fails when a
generated module is out of date.

```bash
make dev && uv run ucbc run bots/foo/a bots/foo/b
make wheels GAME=foo                             # single-game engine + SDK in dist/
```
