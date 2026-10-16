import { useFrame, useSelection } from "@ucbc/viewer";
import { useEffect, useRef } from "react";

import type { Action, Building } from "./api.gen.ts";
import { found, useFind } from "./find.ts";
import { timeline, type World } from "./world.ts";

/** Tile bytes' names and colours, in byte order. */
export const TERRAIN: [string, string][] = [
  ["grass", "#4f7d3a"],
  ["forest", "#2f5a32"],
  ["rock", "#7b7b7b"],
  ["water", "#24508a"],
  ["dirt", "#8c6a48"],
  ["coal", "#262626"],
  ["copper", "#a8706a"],
  ["iron", "#a6aeb8"],
  ["farmland", "#5c4022"],
];
/** The letter drawn on each kind of building once the board is zoomed in. */
export const GLYPHS: Record<Building, string> = {
  base: "B",
  chest: "C",
  gate: "G",
  mech_drill: "M",
  monument: "★",
  radio_tower: "R",
  research_desk: "L",
  smelter: "S",
  spike_trap: "^",
  steam_drill: "D",
  stone_wall: "#",
  tower: "T",
  well: "W",
  wood_wall: "=",
  workbench: "K",
};
/** How each kind of building is drawn while too small for its letter. */
export type Shape = "wall" | "store" | "work" | "heart" | "tower" | "trap";
export const SHAPES: Record<Building, Shape> = {
  base: "heart",
  chest: "store",
  gate: "wall",
  mech_drill: "work",
  monument: "heart",
  radio_tower: "work",
  research_desk: "work",
  smelter: "work",
  spike_trap: "trap",
  steam_drill: "work",
  stone_wall: "wall",
  tower: "tower",
  well: "work",
  wood_wall: "wall",
  workbench: "work",
};
export const MARKS = { crop: "#a8d65c", ripe: "#e0c341", pile: "#f4ecd8", sign: "#ffffff", hit: "#ef4444", found: "#e879f9" };

/** As pixels in an ImageData's buffer, which is little-endian RGBA. */
const PIXELS = TERRAIN.map(
  ([, hex]) =>
    0xff000000 |
    (parseInt(hex.slice(5, 7), 16) << 16) |
    (parseInt(hex.slice(3, 5), 16) << 8) |
    parseInt(hex.slice(1, 3), 16),
);
const OUTLINE = "#111111";

/** Pixels a tile takes: a canvas about 4000 pixels across, at least 4 a tile. */
export function cellSize(size: number): number {
  return Math.max(4, Math.floor(4096 / size));
}

/** The terrain at one pixel a tile, kept between frames and repainted where tiles change. */
interface Terrain {
  canvas: HTMLCanvasElement;
  image: ImageData;
}

const terrains = new WeakMap<World, Terrain>();

function terrain(w: World): Terrain | null {
  let t = terrains.get(w);
  if (!t) {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = w.size;
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    t = { canvas, image: ctx.createImageData(w.size, w.size) };
    terrains.set(w, t);
    w.dirty = null;
  }
  const pixels = new Uint32Array(t.image.data.buffer);
  const paint = (i: number) => (pixels[i] = PIXELS[w.tiles[i]!] ?? 0xffff00ff);
  const ctx = t.canvas.getContext("2d")!;
  if (w.dirty === null) {
    for (let i = 0; i < pixels.length; i++) paint(i);
    ctx.putImageData(t.image, 0, 0);
  } else if (w.dirty.size > 0) {
    let [x0, y0, x1, y1] = [w.size, w.size, 0, 0];
    for (const i of w.dirty) {
      paint(i);
      const [x, y] = [i % w.size, Math.floor(i / w.size)];
      [x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
    }
    ctx.putImageData(t.image, 0, 0, x0, y0, x1 - x0 + 1, y1 - y0 + 1);
  }
  w.dirty = new Set();
  return t;
}

/** Draws `w`, keeping markers a few screen pixels wide however far the board is zoomed
 * out. `hits` are the tiles attacked in the frame's tick; buildings and crops matching
 * `query` are ringed, and other buildings faded. */
function draw(canvas: HTMLCanvasElement, w: World, selected: number | null, hits: Set<number>, query: string) {
  const ctx = canvas.getContext("2d");
  const ground = terrain(w);
  if (!ctx || !ground) return;
  const cell = cellSize(w.size);
  const shown = canvas.getBoundingClientRect().width;
  const px = shown ? canvas.width / shown : 1;
  const css = getComputedStyle(canvas);
  const team = [0, 1].map((i) => css.getPropertyValue(`--ucbc-team-${i}`).trim() || ["#f97316", "#3b82f6"][i]!);
  const centre = (at: number) => [((at % w.size) + 0.5) * cell, (Math.floor(at / w.size) + 0.5) * cell] as const;
  const square = (x: number, y: number, side: number, fill: string, outline = OUTLINE, line = px) => {
    const [cx, cy] = [(x + 0.5) * cell, (y + 0.5) * cell];
    ctx.fillStyle = fill;
    ctx.fillRect(cx - side / 2, cy - side / 2, side, side);
    ctx.strokeStyle = outline;
    ctx.lineWidth = line;
    ctx.strokeRect(cx - side / 2, cy - side / 2, side, side);
  };
  const diamond = (x: number, y: number, side: number, edge: string) => {
    const [cx, cy, r] = [(x + 0.5) * cell, (y + 0.5) * cell, side * 0.7];
    ctx.beginPath();
    ctx.moveTo(cx, cy - r);
    ctx.lineTo(cx + r, cy);
    ctx.lineTo(cx, cy + r);
    ctx.lineTo(cx - r, cy);
    ctx.closePath();
    ctx.fillStyle = OUTLINE;
    ctx.fill();
    ctx.strokeStyle = edge;
    ctx.lineWidth = 1.5 * px;
    ctx.stroke();
  };
  const dot = (at: number, r: number, fill: string) => {
    const [cx, cy] = centre(at);
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, 2 * Math.PI);
    ctx.fillStyle = fill;
    ctx.fill();
    ctx.strokeStyle = OUTLINE;
    ctx.lineWidth = px;
    ctx.stroke();
  };

  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(ground.canvas, 0, 0, w.size * cell, w.size * cell);
  // What the search finds is ringed and the other buildings faded, unless it finds nothing.
  const kinds = found(query);
  const ringed: [number, number][] = [];
  for (const [at, { kind, ripeAt }] of w.crops) {
    if (kinds.has(kind)) ringed.push([at % w.size, Math.floor(at / w.size)]);
    ctx.fillStyle = ripeAt <= w.tick ? MARKS.ripe : MARKS.crop;
    const inset = cell / 6;
    ctx.fillRect((at % w.size) * cell + inset, Math.floor(at / w.size) * cell + inset, cell - 2 * inset, cell - 2 * inset);
  }
  for (const at of w.piles) dot(at, Math.max(cell / 5, 1.5 * px), MARKS.pile);
  for (const at of w.signs) dot(at, Math.max(cell / 6, 1.5 * px), MARKS.sign);
  // Letters once a tile is this many screen pixels across; walls, the most of them, later.
  const lettered = (shape: Shape) => cell / px >= (shape === "wall" ? 10 : 7);
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  for (const s of w.structures.values()) {
    const hit = kinds.has(s.building);
    if (hit) ringed.push([s.x, s.y]);
    ctx.globalAlpha = (s.workLeft > 0 ? 0.45 : 1) * (kinds.size && !hit ? 0.3 : 1);
    const shape = SHAPES[s.building];
    const side = shape === "trap" ? Math.max(cell * 0.6, 4 * px) : Math.max(cell, 7 * px);
    const colour = team[s.team]!;
    if (shape === "trap") diamond(s.x, s.y, side, colour);
    else if (shape === "store") square(s.x, s.y, side, OUTLINE, colour, 2.5 * px);
    else square(s.x, s.y, side, colour, OUTLINE, 1.5 * px);
    if (shape === "wall") {
      ctx.fillStyle = "rgba(0, 0, 0, 0.45)";
      ctx.fillRect((s.x + 0.5) * cell - side / 2, (s.y + 0.5) * cell - side / 2, side, side);
    }
    if (lettered(shape)) {
      const font = `bold ${Math.round(side * 0.7)}px sans-serif`;
      if (ctx.font !== font) ctx.font = font;
      ctx.lineWidth = 2 * px;
      ctx.strokeStyle = OUTLINE;
      ctx.fillStyle = "#ffffff";
      const [cx, cy] = [(s.x + 0.5) * cell, (s.y + 0.55) * cell];
      ctx.strokeText(GLYPHS[s.building], cx, cy);
      ctx.fillText(GLYPHS[s.building], cx, cy);
    } else if (shape === "heart") {
      square(s.x, s.y, side / 3, "#ffffff", "#ffffff", 0);
    } else if (shape === "work" || shape === "tower") {
      dot(s.y * w.size + s.x, side / 5, shape === "work" ? "#ffffff" : OUTLINE);
    }
  }
  ctx.globalAlpha = 1;
  // Units are round, so they don't pass for buildings.
  for (const u of w.units.values()) {
    const r = Math.max(cell * 0.4, 2.5 * px);
    dot(u.y * w.size + u.x, r, team[u.team]!);
    if (u.commander) {
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 2 * px;
      ctx.beginPath();
      ctx.arc((u.x + 0.5) * cell, (u.y + 0.5) * cell, r + 2 * px, 0, 2 * Math.PI);
      ctx.stroke();
    }
  }
  for (const at of hits) {
    const [cx, cy] = centre(at);
    const r = Math.max(cell * 0.6, 5 * px);
    ctx.strokeStyle = MARKS.hit;
    ctx.lineWidth = 2 * px;
    ctx.beginPath();
    ctx.moveTo(cx - r, cy - r);
    ctx.lineTo(cx + r, cy + r);
    ctx.moveTo(cx + r, cy - r);
    ctx.lineTo(cx - r, cy + r);
    ctx.stroke();
  }
  const ring = Math.max(cell * 1.6, 12 * px);
  ctx.beginPath();
  for (const [x, y] of ringed) ctx.rect((x + 0.5) * cell - ring / 2, (y + 0.5) * cell - ring / 2, ring, ring);
  for (const [colour, width] of [[OUTLINE, 4 * px], [MARKS.found, 2 * px]] as const) {
    ctx.strokeStyle = colour;
    ctx.lineWidth = width;
    ctx.stroke();
  }
  const chosen = selected == null ? undefined : w.units.get(selected);
  if (chosen) {
    const outline = (x: number, y: number, side: number) => {
      for (const [colour, width] of [[OUTLINE, 4 * px], ["#ffffff", 2 * px]] as const) {
        ctx.strokeStyle = colour;
        ctx.lineWidth = width;
        ctx.strokeRect(x, y, side, side);
      }
    };
    const r = w.vision;
    outline((chosen.x - r) * cell, (chosen.y - r) * cell, (2 * r + 1) * cell);
    const side = Math.max(cell, 8 * px);
    outline((chosen.x + 0.5) * cell - side / 2, (chosen.y + 0.5) * cell - side / 2, side);
  }
}

/** The map, drawn on a canvas, redrawn on each frame and zoom. Clicking a unit selects
 * it and outlines what it sees; clicking anywhere else clears the selection. Tiles
 * attacked in the frame's tick are crossed in red, and what the search finds is ringed. */
export function Board() {
  const { set, tick } = useFrame();
  const { selected, select } = useSelection();
  const query = useFind();
  const canvas = useRef<HTMLCanvasElement>(null);
  const t = timeline(set);
  const index = tick ? set.ticks.indexOf(tick) + 1 : 0;
  const w = t.worldAt(index);
  const px = w.size * cellSize(w.size);

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const attacks = (tick?.steps ?? []).flatMap((s) => s.actions as Action[]).filter((a) => a.type === "attack");
    const hits = new Set(attacks.map((a) => a.y * w.size + a.x));
    let pending = 0;
    const redraw = () => {
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(() => draw(c, t.worldAt(index), selected, hits, query));
    };
    redraw();
    // The viewer zooms the board's container with a transform.
    const zoom = new MutationObserver(redraw);
    if (c.parentElement) zoom.observe(c.parentElement, { attributes: true, attributeFilter: ["style"] });
    return () => {
      cancelAnimationFrame(pending);
      zoom.disconnect();
    };
  }, [t, index, selected, tick, w.size, query]);

  return (
    <canvas
      ref={canvas}
      className="fr-board"
      width={px}
      height={px}
      onClick={(e) => {
        // The nearest unit to the click, within its tile or a few screen pixels.
        const r = e.currentTarget.getBoundingClientRect();
        const x = ((e.clientX - r.left) / r.width) * w.size;
        const y = ((e.clientY - r.top) / r.height) * w.size;
        const reach = Math.max(0.75, (6 * w.size) / r.width);
        const near = [...t.worldAt(index).units.values()]
          .map((u) => ({ u, d: Math.max(Math.abs(u.x + 0.5 - x), Math.abs(u.y + 0.5 - y)) }))
          .filter(({ d }) => d <= reach)
          .sort((a, b) => a.d - b.d);
        select(near[0]?.u.id ?? null);
      }}
    />
  );
}
