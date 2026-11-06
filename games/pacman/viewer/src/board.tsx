import { useFrame, useSelection } from "@ucbc/viewer";
import { useMemo, type ReactNode } from "react";

import type { Action, AgentView, Coord, Dir } from "./api.gen.ts";
import { useSnapshot } from "./snapshot.ts";
import { CELL, GhostSprite, PacmanSprite, StackBadge, TileSprite } from "./sprites.tsx";

function at({ x, y }: Coord): string {
  return `translate(${x * CELL} ${y * CELL})`;
}

/** The way Pac-Man moved in the drawn tick, or right if it didn't. */
function facing(steps: { bot: number; actions: unknown[] }[], pacman: number): Dir {
  const moves = steps.filter((s) => s.bot === pacman).flatMap((s) => s.actions as Action[]);
  return moves.at(-1)?.dir ?? "right";
}

function Maze({ maze }: { maze: string[] }) {
  return (
    <>
      {maze.flatMap((row, y) =>
        [...row].map((tile, x) =>
          tile === " " ? null : (
            <g key={`${x},${y}`} transform={at({ x, y })}>
              <TileSprite tile={tile} />
            </g>
          ),
        ),
      )}
    </>
  );
}

/** The maze with its pellets, then the ghosts, then Pac-Man on top, and the selection.
 * Clicking an agent selects it; clicking anywhere else clears the selection. */
export function Board() {
  const { tick } = useFrame();
  const s = useSnapshot();
  const { selected, select } = useSelection();
  // Pellets change as Pac-Man eats, so the maze is redrawn when the snapshot changes.
  const maze = useMemo(() => <Maze maze={s.maze} />, [s.maze]);
  const width = (s.maze[0]?.length ?? 0) * CELL;
  const height = s.maze.length * CELL;
  const ghosts = s.agents.filter((a) => a.role === "ghost");
  const pacman = s.agents.find((a) => a.role === "pacman");
  const chosen = s.agents.find((a) => a.id === selected);
  // Ghosts can share a tile and hide each other; a badge says how many are there.
  const stacks = new Map<string, { pos: Coord; count: number }>();
  for (const g of ghosts) {
    const key = `${g.pos.x},${g.pos.y}`;
    stacks.set(key, { pos: g.pos, count: (stacks.get(key)?.count ?? 0) + 1 });
  }

  const agent = (a: AgentView, sprite: ReactNode) => (
    <g key={a.id} className="pm-agent" transform={at(a.pos)} data-bot={a.id}>
      {sprite}
      <rect className="pm-hit" width={CELL} height={CELL} />
    </g>
  );

  return (
    <svg
      className="pm-board"
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      onClick={(e) => {
        const hit = (e.target as Element).closest("[data-bot]");
        select(hit ? Number(hit.getAttribute("data-bot")) : null);
      }}
    >
      <rect className="pm-floor" width={width} height={height} />
      <g className="pm-maze">{maze}</g>
      <g className="pm-ghosts">
        {ghosts.map((g, i) => agent(g, <GhostSprite ghost={g} index={i} tick={s.tick} />))}
      </g>
      <g className="pm-stacks">
        {[...stacks].map(([key, { pos, count }]) =>
          count > 1 ? (
            <g key={key} transform={at(pos)}>
              <StackBadge count={count} />
            </g>
          ) : null,
        )}
      </g>
      {pacman ? agent(pacman, <PacmanSprite facing={facing(tick?.steps ?? [], pacman.id)} />) : null}
      {chosen ? <rect className="pm-selected" transform={at(chosen.pos)} width={CELL} height={CELL} /> : null}
    </svg>
  );
}
