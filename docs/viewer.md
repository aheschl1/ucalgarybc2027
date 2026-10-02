# Viewer

`@ucbc/viewer` (`ucbc-viewer/`) plays a replay and knows no game. Each game adds a
renderer package, `@ucbc/viewer-<game>` in `games/<game>/viewer/`, with a `Board` and an
`Info` component (`GameRenderer` in `src/renderer.ts`). Two hosts render the same
`<Viewer replay renderers />`: the page behind `ucbc view`, and `/viewer/<match id>` in
the web app.

## Who draws what

```text
┌──────────────────────────────────────────────────────────────────────┐
│ header    core: teams, match winner, set score                       │
├──────────────────────────────────────────────────────────────────────┤
│ bar       core: set buttons, this set's verdict                      │
├────────────────────────────────────────┬─────────────────────────────┤
│ stage     core: zoom.ts                │ Info      GAME              │
│  ┌──────────────────────────────────┐  │  scores, selected unit      │
│  │ Board     GAME: places, selects  │  ├─────────────────────────────┤
│  │  ┌────────────────────────────┐  │  │ BotLog    core              │
│  │  │ sprites   SKIN: looks      │  │  │  selected bot's output      │
│  │  └────────────────────────────┘  │  │                             │
│  └──────────────────────────────────┘  │                             │
│                           [− Fit +]    ├─────────────────────────────┤
├────────────────────────────────────────┤ Steps     core              │
│ controls  core: step, play, slider,    │  this tick's steps          │
│           speed                        │                             │
└────────────────────────────────────────┴─────────────────────────────┘
```

Only `Board` and `Info` hold game code. They take no props: `useFrame()` gives the frame
on show and `useSelection()` the selected bot and `select`, which the core owns. Tests
render them inside a `ViewerContext`. A renderer throws on state it does not recognise
and the board blanks for that frame. A game with no renderer shows its state as raw JSON.

## Where to change what

```text
layout, header, bar, controls, keys    ucbc-viewer/src/viewer.tsx, viewer.css
step table, bot log, selection         ucbc-viewer/src/viewer.tsx
frame <-> tick mapping                 ucbc-viewer/src/timeline.ts   (frame 0 = initial_state)
zoom, pan, click vs drag               ucbc-viewer/src/zoom.ts
colours, team colours                  ucbc-viewer/src/viewer.css    (--ucbc-* on .ucbc-viewer)
the renderer contract, hooks           ucbc-viewer/src/renderer.ts, hooks.ts
a game's info panel                    games/<game>/viewer/src/index.tsx
a game's board: placement, clicks      games/<game>/viewer/src/board.tsx
how each thing on the board looks      games/<game>/viewer/src/skin/, assets/    (below)
loading a replay (fetch, file drop)    ucbc-viewer/src/app.tsx
serving it for `ucbc view`             ucbc-cli/python/ucbc/viewer/__init__.py
the web app route                      ucbc-web/src/Viewer.tsx       (assembles a Replay from the API)
which renderers a build bundles        ucbc-viewer/vite/games.ts     (games in ucbc-games `default`)
replay and game types                  generated: ucbc-viewer/scripts/gen-types.mjs
```

`replay.gen.ts` and each game's `api.gen.ts` come from the Rust types (`make
viewer-types`) and are committed; change the Rust and regenerate. `make viewer` builds
the page into the `ucbc` package; `make web` builds the web app, which imports the
viewer as a workspace package. Core classes start `ucbc-`; game classes use their own
prefix (`u27-`, `ttt-`).

## Working on the viewer alone

Node only: `npm ci`, then `npm run dev -w @ucbc/viewer`. With no replay given, the page
lists the sample replays in `games/<game>/viewer/replays/`; a replay file dropped on the
page also works. Saved changes to sprites, css, or pictures show on the open page.

The samples are committed and made by `make replays`. `make test` replays them against
the engine and fails when they are stale; rerun `make replays` then. Keep them few: each
is a few hundred KB, rewritten whenever the game changes.

## Changing how ucbc2027 looks

```text
games/ucbc2027/viewer/assets/        one picture per thing, per team where coloured
games/ucbc2027/viewer/src/skin/
  assets.ts                          which file each thing uses
  sprites.tsx                        one component per thing: picture plus extras (level, held item)
  skin.css                           sprite styling
```

A sprite draws in SVG in a box at the origin: one tile is 16 units, a lab is 2x2 tiles.
Pictures are drawn into that box keeping their aspect ratio. The board places sprites,
takes clicks over a unit's whole footprint, and draws the selection, so a sprite never
handles either. Sprites are listed per generated `type`, so a new tile, item, or unit
type in the Rust fails the type check until it has one.

## Adding a renderer

`games/foo/viewer/` as package `@ucbc/viewer-foo` exporting
`renderer: GameRenderer = { game: "foo", Board, Info }`, then `make viewer-types` for its
`api.gen.ts`. It ships once `foo` is in `default` in `ucbc-games/Cargo.toml`.
