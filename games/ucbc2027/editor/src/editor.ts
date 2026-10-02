import "./editor.css";

import { type Brush, type Mirror, blank, decode, encode, paint } from "./map.ts";
import { Environment, Item } from "./map_pb.ts";

export interface EditorOptions {
  /** Called with the encoded map when Save is pressed. Resolves to what to tell the user
   * if not "Saved"; rejects to report a failure. */
  save(bytes: Uint8Array): Promise<string | void>;
}

export interface Editor {
  /** Replaces the map being edited with a map file. Throws if it is not one. */
  open(bytes: Uint8Array): void;
  destroy(): void;
}

const NEW_SIZE = 16;
// As the engine's `MAX_SIDE` in games/ucbc2027/src/map.rs.
const MAX_SIDE = 64;
const BRUSHES: Brush[] = ["empty", "wall", "lab", "fossil"];
const MIRRORS: [Mirror, string][] = [
  ["none", "No mirror"],
  ["left-right", "Mirror left-right"],
  ["top-bottom", "Mirror top-bottom"],
];
const ENVIRONMENT = {
  [Environment.EMPTY]: "empty",
  [Environment.WALL]: "wall",
  [Environment.LAB]: "lab",
};

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  el.className = className;
  el.append(...children);
  return el;
}

function sizeInput(label: string): HTMLInputElement {
  const input = h("input", "ed-size");
  input.type = "number";
  input.min = "1";
  input.max = String(MAX_SIDE);
  input.value = String(NEW_SIZE);
  input.setAttribute("aria-label", label);
  return input;
}

/** A map editor filling `el`: a toolbar, and the board, as large as fits, painted by
 * clicking or dragging. It starts on an empty 16x16 map. */
export function createEditor(el: HTMLElement, options: EditorOptions): Editor {
  let map = blank(NEW_SIZE, NEW_SIZE);
  let brush: Brush = "wall";
  let mirror: Mirror = "none";
  let painting = false;
  let tiles: HTMLElement[] = [];

  const width = sizeInput("Width");
  const height = sizeInput("Height");
  const newMap = h("button", "ed-button", "New");
  const brushes = h("div", "ed-brushes");
  for (const b of BRUSHES) {
    const pick = h("button", `ed-button ed-brush ed-${b}`, b);
    pick.type = "button";
    pick.dataset.brush = b;
    pick.setAttribute("aria-pressed", String(b === brush));
    brushes.append(pick);
  }
  const mirrors = h("select", "ed-mirror");
  for (const [value, label] of MIRRORS) {
    const option = h("option", "", label);
    option.value = value;
    mirrors.append(option);
  }
  const save = h("button", "ed-button ed-save", "Save");
  for (const b of [newMap, save]) b.type = "button";
  const status = h("span", "ed-status");
  const board = h("div", "ed-board");
  const root = h(
    "div",
    "ed-editor",
    h("div", "ed-toolbar", width, "×", height, newMap, brushes, mirrors, save, status),
    h("div", "ed-stage", board),
  );
  el.append(root);

  function drawTile(i: number) {
    const tile = map.tiles[i]!;
    const fossil = tile.item === Item.FOSSIL ? " ed-fossil" : "";
    tiles[i]!.className = `ed-tile ed-${ENVIRONMENT[tile.environment]}${fossil}`;
  }

  function draw() {
    // The board scales to fit the stage; these give it the map's shape.
    board.style.setProperty("--ed-width", String(map.width));
    board.style.setProperty("--ed-height", String(map.height));
    tiles = map.tiles.map((_, i) => {
      const tile = h("span");
      tile.dataset.i = String(i);
      return tile;
    });
    tiles.forEach((_, i) => drawTile(i));
    board.replaceChildren(...tiles);
    width.value = String(map.width);
    height.value = String(map.height);
  }

  function paintAt(target: EventTarget | null) {
    const i = Number((target as HTMLElement | null)?.dataset?.i ?? NaN);
    if (Number.isNaN(i)) return;
    const painted = paint(map, i % map.width, Math.floor(i / map.width), brush, mirror);
    painted.forEach(drawTile);
    if (painted.length > 0) status.textContent = "Unsaved changes";
  }

  newMap.addEventListener("click", () => {
    const side = (input: HTMLInputElement) => {
      const n = Math.trunc(Number(input.value)) || 1;
      return Math.min(Math.max(n, 1), MAX_SIDE);
    };
    map = blank(side(width), side(height));
    draw();
    status.textContent = "New map, unsaved";
  });
  brushes.addEventListener("click", (e) => {
    const picked = (e.target as HTMLElement).dataset.brush as Brush | undefined;
    if (!picked) return;
    brush = picked;
    for (const b of brushes.children) b.setAttribute("aria-pressed", String(b === e.target));
  });
  mirrors.addEventListener("change", () => (mirror = mirrors.value as Mirror));
  save.addEventListener("click", async () => {
    status.textContent = "Saving…";
    try {
      status.textContent = (await options.save(encode(map))) || "Saved";
    } catch (e) {
      status.textContent = `Not saved: ${e instanceof Error ? e.message : String(e)}`;
    }
  });
  board.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    painting = true;
    paintAt(e.target);
  });
  board.addEventListener("pointerover", (e) => {
    if (painting) paintAt(e.target);
  });
  const stop = () => (painting = false);
  addEventListener("pointerup", stop);

  draw();
  return {
    open(bytes) {
      const opened = decode(bytes);
      if (opened.width < 1 || opened.tiles.length !== opened.width * opened.height) {
        throw new Error(`a ${opened.width}x${opened.height} map with ${opened.tiles.length} tiles`);
      }
      map = opened;
      draw();
      status.textContent = "";
    },
    destroy() {
      removeEventListener("pointerup", stop);
      root.remove();
    },
  };
}
