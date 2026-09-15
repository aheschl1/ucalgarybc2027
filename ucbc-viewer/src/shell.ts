import "./viewer.css";

import type { Replay, Step, TeamId, TeamInfo } from "./replay.gen.ts";
import type { GameRenderer, RendererInstance } from "./renderer.ts";
import { frameAt, frameCount } from "./timeline.ts";

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
    mount(el) {
      const pre = h("pre", "ucbc-raw");
      el.append(h("p", "ucbc-error", `No renderer for game "${game}".`), pre);
      return {
        draw: (frame) => (pre.textContent = JSON.stringify(frame.state, null, 2)),
        destroy: () => el.replaceChildren(),
      };
    },
  };
}

function stepView(step: Step, teams: TeamInfo[]): HTMLElement {
  const usage = [`${(step.usage.time_us / 1000).toFixed(2)} ms`];
  if (step.usage.memory != null) usage.push(`${(step.usage.memory / 2 ** 20).toFixed(1)} MiB`);
  const el = h(
    "div",
    "ucbc-step",
    h(
      "div",
      "ucbc-step-head",
      h("span", "ucbc-team", teamName(teams, step.team)),
      h("span", "ucbc-muted", `bot ${step.bot}`),
      h("span", "ucbc-muted", usage.join(" · ")),
    ),
  );
  for (const action of step.actions) el.append(h("code", "ucbc-action", JSON.stringify(action)));
  if (step.actions.length === 0) el.append(h("span", "ucbc-muted", "no actions"));
  if (step.stdout) el.append(h("pre", "ucbc-stdout", step.stdout));
  if (step.failure) {
    const f = step.failure;
    el.append(h("div", "ucbc-error", `${f.kind}: ${f.message}`));
    if (f.traceback) el.append(h("pre", "ucbc-traceback", f.traceback));
  }
  return el;
}

/** Mounts a replay viewer into `el`. The game's renderer draws the board; the viewer
 * owns sets, playback, and the per-tick step log. */
export function createViewer(el: HTMLElement, options: ViewerOptions): Viewer {
  const root = h("div", "ucbc-viewer");
  root.tabIndex = 0;

  const title = h("div", "ucbc-title");
  const result = h("div", "ucbc-result");
  const sets = h("nav", "ucbc-sets");
  const setResult = h("div", "ucbc-set-result");
  const drawError = h("p", "ucbc-error ucbc-draw-error");
  drawError.hidden = true;
  const stage = h("div", "ucbc-stage");
  const slider = h("input", "ucbc-slider");
  slider.type = "range";
  slider.min = "0";
  const position = h("span", "ucbc-position");
  const play = button("▶", "Play (space)", () => toggle());
  const speed = h("select", "ucbc-speed");
  for (const s of SPEEDS) speed.append(new Option(`${s}/s`, String(s)));
  speed.value = "5";
  const steps = h("section", "ucbc-steps");

  root.append(
    h("header", "ucbc-header", title, result),
    sets,
    setResult,
    drawError,
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
    steps,
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
    steps.replaceChildren(
      ...(f.tick
        ? f.tick.steps.map((s) => stepView(s, replay!.teams))
        : [h("p", "ucbc-muted", "Before the first tick.")]),
    );
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
  }

  function load(next: Replay) {
    stop();
    renderer?.destroy();
    replay = next;
    const game = next.config.game;
    renderer = (options.renderers.find((r) => r.game === game) ?? fallback(game)).mount(stage);

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
    const actions: Record<string, () => void> = {
      ArrowLeft: () => seek(frame - 1),
      ArrowRight: () => seek(frame + 1),
      Home: () => seek(0),
      End: () => seek(Infinity),
      " ": toggle,
    };
    const action = actions[e.key];
    if (!action) return;
    e.preventDefault();
    if (e.key !== " ") stop();
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
