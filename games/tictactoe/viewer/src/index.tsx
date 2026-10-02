import "./board.css";

import type { FrameProps, GameRenderer } from "@ucbc/viewer";

import type { Action, Board as State } from "./api.gen.ts";

const SYMBOL = { empty: "", x: "X", o: "O" } as const;

function Board({ frame: { state, tick } }: FrameProps) {
  const board = state as State;
  if (board?.cells?.length !== 9 || !board.cells.every((c) => Object.hasOwn(SYMBOL, c))) {
    throw new Error(`not a tic-tac-toe board: ${JSON.stringify(state)}`);
  }
  const placed = new Set((tick?.steps ?? []).flatMap((s) => s.actions as Action[]).map((a) => a.row * 3 + a.col));
  return (
    <div className="ttt-board">
      {board.cells.map((cell, i) => (
        <div key={i} className={`ttt-cell ttt-${cell}${placed.has(i) ? " ttt-placed" : ""}`}>
          {SYMBOL[cell]}
        </div>
      ))}
    </div>
  );
}

function Info({ frame: { set, teams } }: FrameProps) {
  const x = teams[set.first_team]?.name ?? "";
  const o = teams.find((t) => t.id !== set.first_team)?.name ?? "";
  return <div className="ucbc-muted">{`X ${x} · O ${o}`}</div>;
}

/** The first team of a set plays X. Cells placed during the drawn tick are highlighted.
 * Every game package exports its renderer under this name; the viewer's build finds it. */
export const renderer: GameRenderer = { game: "tictactoe", Board, Info };
