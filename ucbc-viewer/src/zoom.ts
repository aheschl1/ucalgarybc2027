/** Zoom and pan for the board: `content` is drawn scaled and translated inside the fixed
 * `view`. Wheel zooms about the cursor, dragging pans. */
export interface Zoom {
  /** Scale `content` to fill the view, centred. */
  fit(): void;
  /** Zoom by `factor` about the view's centre. */
  zoomBy(factor: number): void;
}

const MIN_SCALE = 0.1;
const MAX_SCALE = 40;

export function createZoom(view: HTMLElement, content: HTMLElement): Zoom {
  let scale = 1;
  let x = 0;
  let y = 0;

  function apply() {
    content.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  }

  /** Zooms about the point (px, py), in view coordinates, which stays put. */
  function zoomAt(factor: number, px: number, py: number) {
    const next = Math.max(MIN_SCALE, Math.min(scale * factor, MAX_SCALE));
    x = px - ((px - x) * next) / scale;
    y = py - ((py - y) * next) / scale;
    scale = next;
    apply();
  }

  function fit() {
    const w = content.offsetWidth;
    const h = content.offsetHeight;
    // Nothing laid out yet (or no layout at all, as in tests).
    if (!w || !h || !view.clientWidth || !view.clientHeight) {
      scale = 1;
      x = y = 0;
      return apply();
    }
    scale = Math.min(view.clientWidth / w, view.clientHeight / h) * 0.95;
    x = (view.clientWidth - w * scale) / 2;
    y = (view.clientHeight - h * scale) / 2;
    apply();
  }

  view.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const r = view.getBoundingClientRect();
      zoomAt(Math.exp(-e.deltaY / 500), e.clientX - r.left, e.clientY - r.top);
    },
    { passive: false },
  );

  // Buttons overlaid on the view keep their clicks.
  const onButton = (e: Event) => (e.target as Element).closest("button") !== null;
  let drag: { id: number; x: number; y: number } | null = null;
  view.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || onButton(e)) return;
    drag = { id: e.pointerId, x: e.clientX, y: e.clientY };
    view.setPointerCapture(e.pointerId);
    view.classList.add("ucbc-dragging");
  });
  view.addEventListener("pointermove", (e) => {
    if (drag?.id !== e.pointerId) return;
    x += e.clientX - drag.x;
    y += e.clientY - drag.y;
    drag.x = e.clientX;
    drag.y = e.clientY;
    apply();
  });
  const end = (e: PointerEvent) => {
    if (drag?.id !== e.pointerId) return;
    drag = null;
    view.classList.remove("ucbc-dragging");
  };
  view.addEventListener("pointerup", end);
  view.addEventListener("pointercancel", end);
  view.addEventListener("dblclick", (e) => {
    if (!onButton(e)) fit();
  });

  apply();
  return {
    fit,
    zoomBy: (factor) => zoomAt(factor, view.clientWidth / 2, view.clientHeight / 2),
  };
}
