// How each thing on the board looks. A sprite draws one thing in SVG, in a box at the
// origin: one tile of `CELL` units, or 2x2 tiles for a lab. The board places it, takes
// clicks, and draws the selection, so a sprite only draws.

import "./skin.css";

import type { ComponentType } from "react";

import type { Environment, ItemView, UnitEntry } from "../api.gen.ts";
import { assets } from "./assets.ts";

/** A tile's side in SVG units. */
export const CELL = 16;

/** A sprite for each `type` of a generated union, given the thing as prop `P`. A new type
 * in the Rust fails the type check here until it has a sprite. */
type Sprites<U extends { type: string }, P extends string> = {
  [T in U["type"]]: ComponentType<{ [K in P]: Extract<U, { type: T }> }>;
};

type Of<U, T> = Extract<U, { type: T }>;

function forTeam(pictures: string[], team: number): string {
  return pictures[team % pictures.length]!;
}

function Picture({ href, side = 1 }: { href: string; side?: number }) {
  return <image href={href} width={side * CELL} height={side * CELL} />;
}

function EmptyTile() {
  return <Picture href={assets.empty} />;
}

function WallTile() {
  return <Picture href={assets.wall} />;
}

function LabTile({ tile }: { tile: Of<Environment, "lab"> }) {
  return <Picture href={forTeam(assets.labTile, tile.team)} />;
}

function Fossil() {
  return <Picture href={assets.fossil} />;
}

function HeldDino({ item }: { item: Of<ItemView, "dino"> }) {
  return <Picture href={forTeam(assets.dino, item.team)} />;
}

function Lab({ unit }: { unit: Of<UnitEntry, "lab"> }) {
  return <Picture href={forTeam(assets.lab, unit.team)} side={2} />;
}

/** The dino's level on it, and what it holds in the bottom-right quarter. */
function Dino({ unit }: { unit: Of<UnitEntry, "dino"> }) {
  return (
    <>
      <Picture href={forTeam(assets.dino, unit.team)} />
      <text className="u27-level" x={CELL / 2} y={CELL / 2}>
        {unit.level}
      </text>
      {unit.held ? (
        <g className="u27-held" transform={`translate(${CELL / 2} ${CELL / 2}) scale(0.5)`}>
          <ItemSprite item={unit.held} />
        </g>
      ) : null}
    </>
  );
}

const tiles: Sprites<Environment, "tile"> = { empty: EmptyTile, wall: WallTile, lab: LabTile };
const items: Sprites<ItemView, "item"> = { fossil: Fossil, dino: HeldDino };
const units: Sprites<UnitEntry, "unit"> = { lab: Lab, dino: Dino };

// Each draws a thing with the sprite for its type. The tables pair each type with its
// own sprite, which the type checker cannot see through an index, hence the casts.

export function TileSprite({ tile }: { tile: Environment }) {
  const Sprite = tiles[tile.type] as ComponentType<{ tile: Environment }>;
  return <Sprite tile={tile} />;
}

export function ItemSprite({ item }: { item: ItemView }) {
  const Sprite = items[item.type] as ComponentType<{ item: ItemView }>;
  return <Sprite item={item} />;
}

export function UnitSprite({ unit }: { unit: UnitEntry }) {
  const Sprite = units[unit.type] as ComponentType<{ unit: UnitEntry }>;
  return <Sprite unit={unit} />;
}
