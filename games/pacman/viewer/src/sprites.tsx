// How each thing looks. A sprite draws in a CELL-unit box at the origin; the board
// places it.

import type { AgentView, Dir } from "./api.gen.ts";

export const CELL = 16;

/** Ghosts in spawn order take these, as in the arcade: Blinky, Pinky, Inky, Clyde. */
const GHOST_COLOURS = ["#ef4444", "#f472b6", "#22d3ee", "#fb923c"];
/** Ticks left at which a frightened ghost starts flashing. */
const FLASH = 6;
const FACING: Record<Dir, number> = { right: 0, down: 90, left: 180, up: 270 };

export function TileSprite({ tile }: { tile: string }) {
  switch (tile) {
    case "#":
      return <rect className="pm-wall" x={1} y={1} width={CELL - 2} height={CELL - 2} rx={3} />;
    case ".":
      return <circle className="pm-pellet" cx={CELL / 2} cy={CELL / 2} r={1.5} />;
    case "o":
      return <circle className="pm-power" cx={CELL / 2} cy={CELL / 2} r={4} />;
    case "G":
      return <rect className="pm-house" width={CELL} height={CELL} />;
    case " ":
      return null;
    default:
      throw new Error(`not a pacman tile: ${JSON.stringify(tile)}`);
  }
}

/** A disc with its mouth open towards `facing`. */
export function PacmanSprite({ facing }: { facing: Dir }) {
  return (
    <path
      className="pm-pacman"
      d="M8 8 L14 5.6 A6.5 6.5 0 1 0 14 10.4 Z"
      transform={`rotate(${FACING[facing]} 8 8)`}
    />
  );
}

/** The `index`th ghost: its own colour, blue while frightened, flashing white near the end. */
export function GhostSprite({ ghost, index, tick }: { ghost: AgentView; index: number; tick: number }) {
  const scared = ghost.frightened > 0;
  const flashing = scared && ghost.frightened <= FLASH && tick % 2 === 1;
  const fill = flashing ? "#f5f5f4" : scared ? "#1d4ed8" : GHOST_COLOURS[index % GHOST_COLOURS.length];
  // The face, in a colour that shows on the body.
  const face = flashing ? "#ef4444" : "#fde68a";
  return (
    <g className={`pm-ghost${scared ? " pm-frightened" : ""}`}>
      <path d="M2 15 V7 A6 6 0 0 1 14 7 V15 L12 13 L10 15 L8 13 L6 15 L4 13 Z" fill={fill} />
      {scared ? (
        <path className="pm-scared-face" d="M4.5 11.5 L6 10.5 L8 11.5 L10 10.5 L11.5 11.5" stroke={face} />
      ) : null}
      <circle cx={5.5} cy={7} r={2} fill={scared ? "none" : "#ffffff"} />
      <circle cx={10.5} cy={7} r={2} fill={scared ? "none" : "#ffffff"} />
      <circle cx={6} cy={7.3} r={1} fill={scared ? face : "#1e3a8a"} />
      <circle cx={11} cy={7.3} r={1} fill={scared ? face : "#1e3a8a"} />
    </g>
  );
}

/** How many agents share a tile, in its corner. */
export function StackBadge({ count }: { count: number }) {
  return (
    <g className="pm-stack">
      <circle cx={CELL - 3} cy={3} r={3.5} />
      <text x={CELL - 3} y={3}>
        {count}
      </text>
    </g>
  );
}
