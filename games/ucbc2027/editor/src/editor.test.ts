// @vitest-environment jsdom

import { expect, it, vi } from "vitest";

import { createEditor } from "./editor.ts";
import { blank, decode, encode, paint } from "./map.ts";
import { Environment } from "./map_pb.ts";

function setup() {
  // In the document, so a pointerup reaches the window.
  const el = document.body.appendChild(document.createElement("div"));
  const save = vi.fn(async (_bytes: Uint8Array): Promise<string | void> => {});
  const editor = createEditor(el, { save });
  const q = <T extends Element>(selector: string) => el.querySelector<T>(selector)!;
  const tiles = () => [...el.querySelectorAll<HTMLElement>(".ed-tile")];
  const pointer = (type: string, target: Element) =>
    target.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true }));
  const pick = (brush: string) => q<HTMLButtonElement>(`[data-brush="${brush}"]`).click();
  return { el, editor, save, q, tiles, pointer, pick };
}

it("starts on an empty 16x16 map", () => {
  const { tiles } = setup();
  expect(tiles()).toHaveLength(256);
  expect(tiles().every((t) => t.className === "ed-tile ed-empty")).toBe(true);
});

it("paints by clicking and dragging, with the picked brush and mirror", () => {
  const { q, tiles, pointer, pick } = setup();
  pointer("pointerdown", tiles()[0]!);
  pointer("pointerover", tiles()[1]!);
  pointer("pointerup", tiles()[1]!);
  pointer("pointerover", tiles()[2]!); // not painting any more
  expect(tiles().slice(0, 3).map((t) => t.className)).toEqual([
    "ed-tile ed-wall",
    "ed-tile ed-wall",
    "ed-tile ed-empty",
  ]);
  expect(q(".ed-status").textContent).toBe("Unsaved changes");

  pick("lab");
  expect(q(`[data-brush="lab"]`).getAttribute("aria-pressed")).toBe("true");
  expect(q(`[data-brush="wall"]`).getAttribute("aria-pressed")).toBe("false");
  const mirror = q<HTMLSelectElement>(".ed-mirror");
  mirror.value = "left-right";
  mirror.dispatchEvent(new Event("change"));
  pointer("pointerdown", tiles()[16 * 7 + 1]!);
  // Read as the engine does: the first lab in reading order is team 0's.
  for (const [x, y, place] of [
    [1, 7, "team-0 ed-top-left"],
    [2, 8, "team-0 ed-bottom-right"],
    [13, 7, "team-1 ed-top-left"],
    [14, 8, "team-1 ed-bottom-right"],
  ] as const) {
    expect(tiles()[16 * y + x]!.className).toBe(`ed-tile ed-lab ed-${place}`);
  }
  // A third lab is one too many.
  mirror.value = "none";
  mirror.dispatchEvent(new Event("change"));
  pointer("pointerdown", tiles()[16 * 12 + 7]!);
  expect(tiles()[16 * 12 + 7]!.className).toBe("ed-tile ed-lab ed-broken");
  mirror.value = "left-right";
  mirror.dispatchEvent(new Event("change"));

  pick("fossil");
  pointer("pointerdown", tiles()[3]!);
  expect(tiles()[3]!.className).toBe("ed-tile ed-empty ed-fossil");
  expect(tiles()[12]!.className).toBe("ed-tile ed-empty ed-fossil");
});

it("makes a new map of the size asked for, clamped to 1 to 64", () => {
  const { q, tiles } = setup();
  const inputs = [...q<HTMLElement>(".ed-toolbar").querySelectorAll<HTMLInputElement>(".ed-size")];
  inputs[0]!.value = "5";
  inputs[1]!.value = "300";
  q<HTMLButtonElement>(".ed-toolbar .ed-button").click();
  expect(tiles()).toHaveLength(320);
  expect(inputs.map((i) => i.value)).toEqual(["5", "64"]);
});

it("saves the encoded map and reports how it went", async () => {
  const { q, save, tiles, pointer } = setup();
  pointer("pointerdown", tiles()[17]!);
  q<HTMLButtonElement>(".ed-save").click();
  await vi.waitFor(() => expect(q(".ed-status").textContent).toBe("Saved"));
  const saved = decode(save.mock.calls[0]![0]);
  expect([saved.width, saved.height]).toEqual([16, 16]);
  expect(saved.tiles[17]!.environment).toBe(Environment.WALL);

  save.mockResolvedValueOnce("Downloaded map.map");
  q<HTMLButtonElement>(".ed-save").click();
  await vi.waitFor(() => expect(q(".ed-status").textContent).toBe("Downloaded map.map"));

  save.mockRejectedValueOnce(new Error("disk full"));
  q<HTMLButtonElement>(".ed-save").click();
  await vi.waitFor(() => expect(q(".ed-status").textContent).toBe("Not saved: disk full"));
});

it("opens a map file, and refuses one whose tiles do not fit its size", () => {
  const { editor, tiles } = setup();
  const map = blank(3, 2);
  paint(map, 0, 0, "lab", "none");
  paint(map, 2, 1, "fossil", "none");
  editor.open(encode(map));
  expect(tiles().map((t) => t.className)).toEqual([
    "ed-tile ed-lab ed-team-0 ed-top-left",
    "ed-tile ed-lab ed-team-0 ed-top-right",
    "ed-tile ed-empty",
    "ed-tile ed-lab ed-team-0 ed-bottom-left",
    "ed-tile ed-lab ed-team-0 ed-bottom-right",
    "ed-tile ed-empty ed-fossil",
  ]);
  map.tiles.pop();
  expect(() => editor.open(encode(map))).toThrow("a 3x2 map with 5 tiles");
  expect(tiles()).toHaveLength(6);
});

it("destroy removes it", () => {
  const { el, editor } = setup();
  editor.destroy();
  expect(el.children).toHaveLength(0);
  el.remove();
});
