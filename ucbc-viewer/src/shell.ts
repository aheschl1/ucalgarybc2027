import "./viewer.css";

import type { Replay, Step, TeamId, TeamInfo } from "./replay.gen.ts";
import type { GameRenderer, RendererInstance } from "./renderer.ts";
import { frameAt, frameCount } from "./timeline.ts";
import { createZoom } from "./zoom.ts";

export interface ViewerOptions {
  renderers: GameRenderer[];
  replay?: Replay;
}

export interface Viewer {
  load(replay: Replay): void;
  destroy(): void;
}

/** Playback speeds, in frames per second. */
const SPEEDS = [1, 2, 5, 10, 30, 60];

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

function button(label: string, title: string, onClick: () => void): HTMLButtonElement {
  const b = h("button", "ucbc-button", label);
  b.type = "button";
  b.title = title;
  b.addEventListener("click", onClick);
  return b;
}

function teamName(teams: TeamInfo[], id: TeamId): string {
  return teams[id]?.name ?? `team ${id}`;
}

/** Shows any state as JSON; used when no renderer matches the replay's game. */
function fallback(game: string): GameRenderer {
  return {
    game,
    mount(board, info) {
      const pre = h("pre", "ucbc-raw");
      board.append(pre);
      info.append(h("div", "ucbc-error", `No renderer for game "${game}".`));
      return {
        draw: (frame) => (pre.textContent = JSON.stringify(frame.state, null, 2)),
        destroy: () => {
          board.replaceChildren();
          info.replaceChildren();
        },
      };
    },
  };
}

/** An action as its `type`, if it has one, then `key=value` for each other field. */
export function formatAction(action: unknown): string {
  if (typeof action !== "object" || action === null || Array.isArray(action)) return JSON.stringify(action);
  const { type, ...fields } = action as Record<string, unknown>;
  const parts = Object.entries(fields).map(([k, v]) => `${k}=${typeof v === "string" ? v : JSON.stringify(v)}`);
  return [...(type === undefined ? [] : [String(type)]), ...parts].join(" ");
}

/** The steps table's columns: heading and class. */
const STEP_COLUMNS = [
  ["Team", ""],
  ["Bot", "ucbc-num"],
  ["ms", "ucbc-num"],
  ["MiB", "ucbc-num"],
  ["Actions", ""],
] as const;

/** A cell spanning the whole steps table. */
function wide(...children: (Node | string)[]): HTMLTableCellElement {
  const td = h("td", "", ...children);
  td.colSpan = STEP_COLUMNS.length;
  return td;
}

/** One step as a table body: a row of numbers, then a row for any output. */
function stepView(step: Step, teams: TeamInfo[]): HTMLElement {
  const swatch = h("span", "ucbc-swatch");
  swatch.style.background = `var(--ucbc-team-${step.team}, var(--ucbc-muted))`;
  const memory = step.usage.memory;
  const actions = step.actions.length
    ? step.actions.map((a) => h("code", "ucbc-action", formatAction(a)))
    : [h("span", "ucbc-muted", "none")];
  const el = h(
    "tbody",
    "ucbc-step",
    h(
      "tr",
      "",
      h("td", "ucbc-team", swatch, teamName(teams, step.team)),
      h("td", "ucbc-num", String(step.bot)),
      h("td", "ucbc-num", (step.usage.time_us / 1000).toFixed(2)),
      h("td", "ucbc-num", memory == null ? "" : (memory / 2 ** 20).toFixed(1)),
      h("td", "", h("div", "ucbc-actions", ...actions)),
    ),
  );
  const output: HTMLElement[] = [];
  if (step.stdout) output.push(h("pre", "ucbc-stdout", step.stdout));
  if (step.failure) {
    const f = step.failure;
    output.push(h("div", "ucbc-error", `${f.kind}: ${f.message}`));
    if (f.traceback) output.push(h("pre", "ucbc-traceback", f.traceback));
  }
  if (output.length) el.append(h("tr", "ucbc-step-output", wide(...output)));
  return el;
}

/** Mounts a replay viewer into `el`. The game's renderer draws the board and its info
 * panel; the viewer owns sets, playback, zoom, and the per-tick step table. */
export function createViewer(el: HTMLElement, options: ViewerOptions): Viewer {
  const root = h("div", "ucbc-viewer");
  root.tabIndex = 0;

  const title = h("div", "ucbc-title");
  const result = h("div", "ucbc-result");
  const sets = h("nav", "ucbc-sets");
  const setResult = h("div", "ucbc-set-result");
  const drawError = h("div", "ucbc-error ucbc-draw-error");
  drawError.hidden = true;
  const board = h("div", "ucbc-board");
  const stage = h("div", "ucbc-stage", board);
  const zoom = createZoom(stage, board);
  stage.append(
    h(
      "div",
      "ucbc-zoom",
      button("−", "Zoom out (-)", () => zoom.zoomBy(1 / 1.25)),
      button("Fit", "Fit (0, double-click)", () => zoom.fit()),
      button("+", "Zoom in (+)", () => zoom.zoomBy(1.25)),
    ),
    drawError,
  );
  const info = h("div", "ucbc-info");
  const slider = h("input", "ucbc-slider");
  slider.type = "range";
  slider.min = "0";
  const position = h("span", "ucbc-position");
  const play = button("▶", "Play (space)", () => toggle());
  const speed = h("select", "ucbc-speed");
  for (const s of SPEEDS) speed.append(new Option(`${s}/s`, String(s)));
  speed.value = "5";
  const steps = h("table", "ucbc-steps");
  const stepsHead = h("thead", "", h("tr", "", ...STEP_COLUMNS.map(([c, cls]) => h("th", cls, c))));

  root.append(
    h("header", "ucbc-header", title, result),
    h("div", "ucbc-bar", sets, setResult),
    stage,
    h(
      "div",
      "ucbc-controls",
      button("⏮", "First (Home)", () => seek(0)),
      button("‹", "Previous (←)", () => seek(frame - 1)),
      play,
      button("›", "Next (→)", () => seek(frame + 1)),
      button("⏭", "Last (End)", () => seek(Infinity)),
      slider,
      position,
      speed,
    ),
    h("aside", "ucbc-side", info, steps),
  );
  el.append(root);

  let replay: Replay | null = null;
  let renderer: RendererInstance | null = null;
  let setIndex = 0;
  let frame = 0;
  let timer: ReturnType<typeof setInterval> | null = null;

  function currentSet() {
    return replay?.sets[setIndex];
  }

  function draw() {
    const set = currentSet();
    if (!replay || !set || !renderer) return;
    const f = frameAt(set, replay.teams, frame);
    // A renderer throws on state it does not recognise, e.g. a replay from an older engine.
    try {
      renderer.draw(f);
      drawError.hidden = true;
    } catch (e) {
      drawError.textContent = `Cannot draw this frame: ${e instanceof Error ? e.message : String(e)}`;
      drawError.hidden = false;
    }
    slider.value = String(frame);
    position.textContent = f.tick ? `tick ${frame} / ${set.ticks.length}` : `start / ${set.ticks.length}`;
    const rows = f.tick
      ? f.tick.steps.map((s) => stepView(s, replay!.teams))
      : [h("tbody", "", h("tr", "", wide(h("span", "ucbc-muted", "Before the first tick."))))];
    steps.replaceChildren(stepsHead, ...rows);
  }

  function seek(to: number) {
    const set = currentSet();
    if (!set) return;
    frame = Math.max(0, Math.min(to, frameCount(set) - 1));
    if (frame === frameCount(set) - 1) stop();
    draw();
  }

  function stop() {
    if (timer !== null) clearInterval(timer);
    timer = null;
    play.textContent = "▶";
  }

  function start() {
    const set = currentSet();
    if (!set) return;
    stop();
    if (frame === frameCount(set) - 1) seek(0);
    timer = setInterval(() => seek(frame + 1), 1000 / Number(speed.value));
    play.textContent = "⏸";
  }

  function toggle() {
    if (timer === null) start();
    else stop();
  }

  function selectSet(index: number) {
    const set = replay?.sets[index];
    if (!replay || !set) return;
    stop();
    setIndex = index;
    frame = 0;
    for (const [i, b] of [...sets.children].entries()) b.classList.toggle("ucbc-active", i === index);
    const r = set.result;
    const verdict =
      r.winner_team == null ? "draw" : `${teamName(replay.teams, r.winner_team)} wins by ${r.reason}`;
    setResult.textContent = `Set ${index + 1}: ${verdict} after ${r.ticks} ticks${r.detail ? ` (${r.detail})` : ""}`;
    slider.max = String(frameCount(set) - 1);
    draw();
    zoom.fit();
  }

  function load(next: Replay) {
    stop();
    renderer?.destroy();
    replay = next;
    const game = next.config.game;
    renderer = (options.renderers.find((r) => r.game === game) ?? fallback(game)).mount(board, info);

    title.textContent = next.teams.map((t) => t.name).join(" vs ");
    const score = next.teams.map((t) => `${t.name} ${next.result.set_wins[t.id] ?? 0}`).join(", ");
    const winner = next.result.winner_team;
    result.textContent = `${winner == null ? "Tie" : `${teamName(next.teams, winner)} wins`} (${score})`;
    sets.replaceChildren(...next.sets.map((s, i) => button(`Set ${s.index + 1}`, "", () => selectSet(i))));
    selectSet(0);
  }

  slider.addEventListener("input", () => {
    stop();
    seek(Number(slider.value));
  });
  speed.addEventListener("change", () => {
    if (timer !== null) start();
  });
  root.addEventListener("keydown", (e) => {
    const target = e.target as HTMLElement;
    const arrows = ["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key);
    // Let focused controls keep their own keys.
    if (target.tagName === "SELECT") return;
    if (target.tagName === "INPUT" && arrows) return;
    if (target.tagName === "BUTTON" && (e.key === " " || e.key === "Enter")) return;
    // Stepping keys pause playback; space and the zoom keys do not.
    const stepping: Record<string, () => void> = {
      ArrowLeft: () => seek(frame - 1),
      ArrowRight: () => seek(frame + 1),
      Home: () => seek(0),
      End: () => seek(Infinity),
    };
    const other: Record<string, () => void> = {
      " ": toggle,
      "+": () => zoom.zoomBy(1.25),
      "=": () => zoom.zoomBy(1.25),
      "-": () => zoom.zoomBy(1 / 1.25),
      "0": () => zoom.fit(),
    };
    const action = stepping[e.key] ?? other[e.key];
    if (!action) return;
    e.preventDefault();
    if (stepping[e.key]) stop();
    action();
  });

  if (options.replay) load(options.replay);

  return {
    load,
    destroy() {
      stop();
      renderer?.destroy();
      root.remove();
    },
  };
}
