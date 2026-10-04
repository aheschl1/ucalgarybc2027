/** Zoom and pan for the board: `content` is drawn scaled and translated inside the fixed
 * `view`. Wheel zooms about the cursor, dragging pans; a press that does not move is a
 * click on the content. */
export interface Zoom {
  /** Scale `content` to fill the view, centred. */
  fit(): void;
  /** Zoom by `factor` about the view's centre. */
  zoomBy(factor: number): void;
  /** Removes the listeners from `view`. */
  destroy(): void;
}

const MIN_SCALE = 0.1;
const MAX_SCALE = 40;
/** How far a press moves before it pans instead of clicking. */
const DRAG_PX = 4;

export function createZoom(view: HTMLElement, content: HTMLElement): Zoom {
  let scale = 1;
  let x = 0;
  let y = 0;
  const listeners = new AbortController();
  const { signal } = listeners;

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
    { passive: false, signal },
  );

  // Buttons overlaid on the view keep their clicks.
  const onButton = (e: Event) => (e.target as Element).closest("button") !== null;
  // A press becomes a drag once it moves; until then it may still be a click on the content.
  let press: { id: number; x: number; y: number; dragging: boolean } | null = null;
  let dragged = false;
  view.addEventListener(
    "pointerdown",
    (e) => {
      dragged = false;
      if (e.button !== 0 || onButton(e)) return;
      press = { id: e.pointerId, x: e.clientX, y: e.clientY, dragging: false };
    },
    { signal },
  );
  view.addEventListener(
    "pointermove",
    (e) => {
      if (press?.id !== e.pointerId) return;
      const dx = e.clientX - press.x;
      const dy = e.clientY - press.y;
      if (!press.dragging) {
        if (Math.hypot(dx, dy) < DRAG_PX) return;
        press.dragging = true;
        view.setPointerCapture(e.pointerId);
        view.classList.add("ucbc-dragging");
      }
      x += dx;
      y += dy;
      press.x = e.clientX;
      press.y = e.clientY;
      apply();
    },
    { signal },
  );
  const end = (e: PointerEvent) => {
    if (press?.id !== e.pointerId) return;
    dragged = press.dragging;
    press = null;
    view.classList.remove("ucbc-dragging");
  };
  view.addEventListener("pointerup", end, { signal });
  view.addEventListener("pointercancel", end, { signal });
  // The click that ends a drag does not reach the content.
  view.addEventListener(
    "click",
    (e) => {
      if (dragged) e.stopPropagation();
      dragged = false;
    },
    { capture: true, signal },
  );
  view.addEventListener(
    "dblclick",
    (e) => {
      if (!onButton(e)) fit();
    },
    { signal },
  );

  apply();
  return {
    fit,
    zoomBy: (factor) => zoomAt(factor, view.clientWidth / 2, view.clientHeight / 2),
    destroy: () => listeners.abort(),
  };
}
