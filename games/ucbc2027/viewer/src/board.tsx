import { useFrame, useSelection } from "@ucbc/viewer";
import { useMemo } from "react";

import type { Coord, Environment, ItemView, UnitEntry } from "./api.gen.ts";
import { CELL, ItemSprite, TileSprite, UnitSprite } from "./skin/sprites.tsx";
import { useSnapshot } from "./snapshot.ts";

/** Tiles a unit covers on each side; a lab's footprint is `lab_footprint` in map.rs. */
const SIDE: Record<UnitEntry["type"], number> = { lab: 2, dino: 1 };
/** Units of a lower layer are drawn first. */
const LAYER: Record<UnitEntry["type"], number> = { lab: 0, dino: 1 };
const FOSSIL: ItemView = { type: "fossil" };

/** A unit's top-left tile. */
function corner(unit: UnitEntry): Coord {
  return unit.type === "dino" ? unit.pos : unit.origin;
}

/** The tiles a unit sees, `vision` out from its footprint and clipped to the board. */
function sight(unit: UnitEntry, cols: number, rows: number) {
  const { x, y } = corner(unit);
  const far = SIDE[unit.type] - 1 + unit.vision;
  const left = Math.max(x - unit.vision, 0);
  const top = Math.max(y - unit.vision, 0);
  const right = Math.min(x + far, cols - 1);
  const bottom = Math.min(y + far, rows - 1);
  return { left, top, width: right - left + 1, height: bottom - top + 1 };
}

function at({ x, y }: Coord): string {
  return `translate(${x * CELL} ${y * CELL})`;
}

function Terrain({ environment }: { environment: Environment[][] }) {
  return (
    <>
      {environment.flatMap((row, y) =>
        row.map((tile, x) => (
          <g key={`${x},${y}`} transform={at({ x, y })}>
            <TileSprite tile={tile} />
          </g>
        )),
      )}
    </>
  );
}

/** The board in SVG layers: terrain, fossils lying on it, units, and the selection with
 * the square of tiles it can see. The
 * skin's sprites draw each thing; the board places them. Clicking a unit selects it, and
 * clicking anywhere else clears the selection. */
export function Board() {
  const { set } = useFrame();
  const s = useSnapshot();
  const { selected, select } = useSelection();
  // The environment is fixed for a set, so the terrain is drawn once per set.
  const terrain = useMemo(() => <Terrain environment={s.environment} />, [set]);
  const cols = s.environment[0]?.length ?? 0;
  const rows = s.environment.length;
  const width = cols * CELL;
  const height = rows * CELL;
  const units = [...s.units].sort((a, b) => LAYER[a.type] - LAYER[b.type]);
  const chosen = s.units.find((u) => u.id === selected);
  const seen = chosen && sight(chosen, cols, rows);

  return (
    <svg
      className="u27-board"
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      onClick={(e) => {
        const unit = (e.target as Element).closest("[data-bot]");
        select(unit ? Number(unit.getAttribute("data-bot")) : null);
      }}
    >
      <g className="u27-terrain">{terrain}</g>
      <g className="u27-items">
        {s.fossils.map((tile, i) => (
          <g key={i} transform={at(tile)}>
            <ItemSprite item={FOSSIL} />
          </g>
        ))}
      </g>
      <g className="u27-units">
        {units.map((unit) => (
          <g key={unit.id} className="u27-bot" transform={at(corner(unit))} data-bot={unit.id}>
            <UnitSprite unit={unit} />
            <rect className="u27-hit" width={SIDE[unit.type] * CELL} height={SIDE[unit.type] * CELL} />
          </g>
        ))}
      </g>
      <g className="u27-overlay">
        {seen ? (
          <rect
            className="u27-vision"
            transform={at({ x: seen.left, y: seen.top })}
            width={seen.width * CELL}
            height={seen.height * CELL}
          />
        ) : null}
        {chosen ? (
          <rect
            className="u27-selected"
            transform={at(corner(chosen))}
            width={SIDE[chosen.type] * CELL}
            height={SIDE[chosen.type] * CELL}
          />
        ) : null}
      </g>
    </svg>
  );
}
