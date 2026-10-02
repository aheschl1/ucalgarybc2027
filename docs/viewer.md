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
│  │ Board     GAME                   │  ├─────────────────────────────┤
│  │  in an error boundary            │  │ BotLog    core              │
│  └──────────────────────────────────┘  │  selected bot's output      │
│                           [− Fit +]    ├─────────────────────────────┤
├────────────────────────────────────────┤ Steps     core              │
│ controls  core: step, play, slider,    │  this tick's steps          │
│           speed                        │                             │
└────────────────────────────────────────┴─────────────────────────────┘
```

Only `Board` and `Info` hold game code. `Board` gets `onSelect`; the core owns the
selected bot. A renderer throws on state it does not recognise and the board blanks for
that frame. A game with no renderer shows its state as raw JSON.

## Where to change what

```text
layout, header, bar, controls, keys    ucbc-viewer/src/viewer.tsx, viewer.css
step table, bot log, selection         ucbc-viewer/src/viewer.tsx
frame <-> tick mapping                 ucbc-viewer/src/timeline.ts   (frame 0 = initial_state)
zoom, pan, click vs drag               ucbc-viewer/src/zoom.ts
colours, team colours                  ucbc-viewer/src/viewer.css    (--ucbc-* on .ucbc-viewer)
the renderer contract                  ucbc-viewer/src/renderer.ts
a game's board and info panel          games/<game>/viewer/src/index.tsx and its css
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

## Adding a renderer

`games/foo/viewer/` as package `@ucbc/viewer-foo` exporting
`renderer: GameRenderer = { game: "foo", Board, Info }`, then `make viewer-types` for its
`api.gen.ts`. It ships once `foo` is in `default` in `ucbc-games/Cargo.toml`.
