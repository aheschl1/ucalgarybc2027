// @vitest-environment jsdom

import { expect, it, vi } from "vitest";

import { createZoom } from "./zoom.ts";

function setup() {
  const view = document.createElement("div");
  const content = view.appendChild(document.createElement("div"));
  const tile = content.appendChild(document.createElement("span"));
  view.setPointerCapture = () => {};
  document.body.append(view);
  const zoom = createZoom(view, content);
  const onClick = vi.fn();
  tile.addEventListener("click", onClick);
  const pointer = (type: string, x: number) =>
    tile.dispatchEvent(new PointerEvent(type, { pointerId: 1, button: 0, clientX: x, clientY: 0, bubbles: true }));
  const press = (to: number) => {
    pointer("pointerdown", 0);
    pointer("pointermove", to);
    pointer("pointerup", to);
    tile.click();
  };
  return { content, zoom, onClick, press };
}

it("clicks through a press that barely moves", () => {
  const { content, onClick, press } = setup();
  press(2);
  expect(onClick).toHaveBeenCalledOnce();
  expect(content.style.transform).toBe("translate(0px, 0px) scale(1)");
});

it("pans on a drag and swallows the click that ends it", () => {
  const { content, onClick, press } = setup();
  press(30);
  expect(content.style.transform).toBe("translate(30px, 0px) scale(1)");
  expect(onClick).not.toHaveBeenCalled();
  // Only that click.
  press(0);
  expect(onClick).toHaveBeenCalledOnce();
});

it("stops listening once destroyed", () => {
  const { content, zoom, press } = setup();
  zoom.destroy();
  press(30);
  expect(content.style.transform).toBe("translate(0px, 0px) scale(1)");
});
