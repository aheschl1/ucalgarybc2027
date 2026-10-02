import "./viewer.css";

import { memo, useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";

import type { GameRenderer } from "./renderer.ts";
import type { BotId, Replay, SetReplay, Step, TeamId, TeamInfo, Tick } from "./replay.gen.ts";
import { frameAt, frameCount } from "./timeline.ts";
import { createZoom, type Zoom } from "./zoom.ts";

export interface ViewerProps {
  replay: Replay;
  renderers: GameRenderer[];
}

/** Playback speeds, in frames per second. */
const SPEEDS = [1, 2, 5, 10, 30, 60];

function teamName(teams: TeamInfo[], id: TeamId): string {
  return teams[id]?.name ?? `team ${id}`;
}

/** An action as its `type`, if it has one, then `key=value` for each other field. */
export function formatAction(action: unknown): string {
  if (typeof action !== "object" || action === null || Array.isArray(action)) return JSON.stringify(action);
  const { type, ...fields } = action as Record<string, unknown>;
  const parts = Object.entries(fields).map(([k, v]) => `${k}=${typeof v === "string" ? v : JSON.stringify(v)}`);
  return [...(type === undefined ? [] : [String(type)]), ...parts].join(" ");
}

function Button(props: { title?: string; active?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      className={props.active ? "ucbc-button ucbc-active" : "ucbc-button"}
      title={props.title}
      onClick={props.onClick}
    >
      {props.children}
    </button>
  );
}

/** The steps table's columns: heading and class. */
const STEP_COLUMNS = [
  ["Team", ""],
  ["Bot", "ucbc-num"],
  ["ms", "ucbc-num"],
  ["MiB", "ucbc-num"],
  ["Actions", ""],
] as const;

/** What a step printed, and how it failed, if it did. */
function Output({ step }: { step: Step }) {
  const f = step.failure;
  return (
    <>
      {step.stdout ? <pre className="ucbc-stdout">{step.stdout}</pre> : null}
      {f ? <div className="ucbc-error">{`${f.kind}: ${f.message}`}</div> : null}
      {f?.traceback ? <pre className="ucbc-traceback">{f.traceback}</pre> : null}
    </>
  );
}

interface StepRowsProps {
  step: Step;
  teams: TeamInfo[];
  selected: boolean;
  onSelect(bot: BotId): void;
}

/** One step as a table body: a row of numbers, then a row for any output. Clicking it
 * selects the bot. */
function StepRows({ step, teams, selected, onSelect }: StepRowsProps) {
  const memory = step.usage.memory;
  return (
    <tbody className={selected ? "ucbc-step ucbc-selected" : "ucbc-step"} onClick={() => onSelect(step.bot)}>
      <tr>
        <td className="ucbc-team">
          <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${step.team}, var(--ucbc-muted))` }} />
          {teamName(teams, step.team)}
        </td>
        <td className="ucbc-num">{step.bot}</td>
        <td className="ucbc-num">{(step.usage.time_us / 1000).toFixed(2)}</td>
        <td className="ucbc-num">{memory == null ? "" : (memory / 2 ** 20).toFixed(1)}</td>
        <td>
          <div className="ucbc-actions">
            {step.actions.length ? (
              step.actions.map((a, i) => (
                <code key={i} className="ucbc-action">
                  {formatAction(a)}
                </code>
              ))
            ) : (
              <span className="ucbc-muted">none</span>
            )}
          </div>
        </td>
      </tr>
      {step.stdout || step.failure ? (
        <tr className="ucbc-step-output">
          <td colSpan={STEP_COLUMNS.length}>
            <Output step={step} />
          </td>
        </tr>
      ) : null}
    </tbody>
  );
}

interface StepsProps {
  tick: Tick | null;
  teams: TeamInfo[];
  selected: BotId | null;
  onSelect(bot: BotId): void;
}

function Steps({ tick, teams, selected, onSelect }: StepsProps) {
  return (
    <table className="ucbc-steps">
      <thead>
        <tr>
          {STEP_COLUMNS.map(([heading, cls]) => (
            <th key={heading} className={cls}>
              {heading}
            </th>
          ))}
        </tr>
      </thead>
      {tick ? (
        tick.steps.map((s, i) => (
          <StepRows key={i} step={s} teams={teams} selected={s.bot === selected} onSelect={onSelect} />
        ))
      ) : (
        <tbody>
          <tr>
            <td colSpan={STEP_COLUMNS.length}>
              <span className="ucbc-muted">Before the first tick.</span>
            </td>
          </tr>
        </tbody>
      )}
    </table>
  );
}

/** A bot's step, and the frame it produced. */
interface Entry {
  frame: number;
  step: Step;
}

/** Each bot's steps in a set, in order. */
function stepsByBot(set: SetReplay): Map<BotId, Entry[]> {
  const bots = new Map<BotId, Entry[]>();
  set.ticks.forEach((tick, i) => {
    for (const step of tick.steps) {
      let entries = bots.get(step.bot);
      if (!entries) bots.set(step.bot, (entries = []));
      entries.push({ frame: i + 1, step });
    }
  });
  return bots;
}

const LogEntry = memo(function LogEntry(props: { entry: Entry; when: number; onSeek(frame: number): void }) {
  const { entry, when, onSeek } = props;
  const cls = when < 0 ? "" : when === 0 ? " ucbc-current" : " ucbc-future";
  return (
    <div className={`ucbc-entry${cls}`}>
      <button type="button" className="ucbc-link" onClick={() => onSeek(entry.frame)}>
        {`tick ${entry.frame}`}
      </button>
      <Output step={entry.step} />
    </div>
  );
});

interface BotLogProps {
  bot: BotId;
  steps: Entry[];
  frame: number;
  teams: TeamInfo[];
  onSeek(frame: number): void;
  onClose(): void;
}

/** A bot's output over the set: each step that printed or failed. Later ones are dimmed,
 * and the log scrolls to keep the latest one at or before `frame` in view. */
function BotLog({ bot, steps, frame, teams, onSeek, onClose }: BotLogProps) {
  const log = useRef<HTMLDivElement>(null);
  const output = steps.filter((e) => e.step.stdout || e.step.failure);
  let latest = -1;
  for (const [i, e] of output.entries()) if (e.frame <= frame) latest = i;
  const team = steps[0]?.step.team;

  useEffect(() => {
    const box = log.current;
    const el = box?.children[latest] as HTMLElement | undefined;
    if (!box) return;
    // The entry's bottom at the box's bottom, or its top at the top if it is taller.
    box.scrollTop = el ? Math.min(el.offsetTop, el.offsetTop + el.offsetHeight - box.clientHeight) : 0;
  }, [bot, latest]);

  return (
    <section className="ucbc-bot">
      <header className="ucbc-bot-header">
        <span className="ucbc-team">
          {team == null ? null : (
            <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${team}, var(--ucbc-muted))` }} />
          )}
          {team == null ? `Bot ${bot}` : `Bot ${bot} · ${teamName(teams, team)}`}
        </span>
        <Button title="Clear selection (Escape)" onClick={onClose}>
          ×
        </Button>
      </header>
      {output.length ? (
        <div className="ucbc-bot-log" ref={log}>
          {output.map((e) => (
            <LogEntry key={e.frame} entry={e} when={Math.sign(e.frame - frame)} onSeek={onSeek} />
          ))}
        </div>
      ) : (
        <div className="ucbc-muted">No output in this set.</div>
      )}
    </section>
  );
}

/** Zoom and pan of the `board` element inside the `stage` element. */
function useZoom() {
  const stage = useRef<HTMLDivElement>(null);
  const board = useRef<HTMLDivElement>(null);
  const zoom = useRef<Zoom>(null);
  useEffect(() => {
    const z = createZoom(stage.current!, board.current!);
    zoom.current = z;
    return () => z.destroy();
  }, []);
  return { stage, board, zoom };
}

/** Plays a replay. The game's renderer draws the board and its info panel; the viewer owns
 * sets, playback, zoom, the per-tick step table, and the selected bot and its output. Give it a new `key` to start another
 * replay from the beginning. */
export function Viewer({ replay, renderers }: ViewerProps) {
  const [setIndex, setSetIndex] = useState(0);
  const [frame, setFrame] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(5);
  const [selected, setSelected] = useState<BotId | null>(null);
  const [drawError, setDrawError] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const { stage, board, zoom } = useZoom();

  const { teams } = replay;
  const game = replay.config.game;
  const renderer = renderers.find((r) => r.game === game);
  const set = replay.sets[setIndex]!;
  const last = frameCount(set) - 1;
  const current = frameAt(set, teams, frame);
  const byBot = useMemo(() => stepsByBot(set), [set]);

  function seek(to: number) {
    setFrame(Math.max(0, Math.min(to, last)));
  }

  function toggle() {
    if (playing) return setPlaying(false);
    if (frame === last) setFrame(0);
    setPlaying(true);
  }

  // Seeking from the bot log, like the stepping keys, pauses.
  const seekPaused = useCallback((to: number) => {
    setPlaying(false);
    setFrame(to);
  }, []);

  function selectSet(index: number) {
    setPlaying(false);
    setSetIndex(index);
    setFrame(0);
    // Bot ids start over in each set.
    setSelected(null);
  }

  useEffect(() => {
    root.current?.focus({ preventScroll: true });
  }, []);

  useEffect(() => {
    zoom.current?.fit();
  }, [setIndex]);

  useEffect(() => {
    if (!playing) return;
    const timer = setInterval(() => setFrame((f) => Math.min(f + 1, last)), 1000 / speed);
    return () => clearInterval(timer);
  }, [playing, speed, last]);

  // Reaching the last frame, by playing or seeking, stops playback.
  useEffect(() => {
    if (frame === last) setPlaying(false);
  }, [frame, last]);

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
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
      "+": () => zoom.current?.zoomBy(1.25),
      "=": () => zoom.current?.zoomBy(1.25),
      "-": () => zoom.current?.zoomBy(1 / 1.25),
      "0": () => zoom.current?.fit(),
      Escape: () => setSelected(null),
    };
    const action = stepping[e.key] ?? other[e.key];
    if (!action) return;
    e.preventDefault();
    if (stepping[e.key]) setPlaying(false);
    action();
  }

  const winner = replay.result.winner_team;
  const score = teams.map((t) => `${t.name} ${replay.result.set_wins[t.id] ?? 0}`).join(", ");
  const r = set.result;
  const verdict = r.winner_team == null ? "draw" : `${teamName(teams, r.winner_team)} wins by ${r.reason}`;

  return (
    <div className="ucbc-viewer" tabIndex={0} ref={root} onKeyDown={onKeyDown}>
      <header className="ucbc-header">
        <div className="ucbc-title">{teams.map((t) => t.name).join(" vs ")}</div>
        <div className="ucbc-result">{`${winner == null ? "Tie" : `${teamName(teams, winner)} wins`} (${score})`}</div>
      </header>
      <div className="ucbc-bar">
        <nav className="ucbc-sets">
          {replay.sets.map((s, i) => (
            <Button key={s.index} active={i === setIndex} onClick={() => selectSet(i)}>
              {`Set ${s.index + 1}`}
            </Button>
          ))}
        </nav>
        <div className="ucbc-set-result">
          {`Set ${setIndex + 1}: ${verdict} after ${r.ticks} ticks${r.detail ? ` (${r.detail})` : ""}`}
        </div>
      </div>
      <div className="ucbc-stage" ref={stage}>
        <div className="ucbc-board" ref={board}>
          {/* A throwing renderer blanks the board and shows why, until the next frame. */}
          <ErrorBoundary
            fallback={null}
            resetKeys={[current.state]}
            onError={(e) => setDrawError(`Cannot draw this frame: ${e instanceof Error ? e.message : String(e)}`)}
            onReset={() => setDrawError("")}
          >
            {renderer ? (
              <renderer.Board frame={current} selected={selected} onSelect={setSelected} />
            ) : (
              <pre className="ucbc-raw">{JSON.stringify(current.state, null, 2)}</pre>
            )}
          </ErrorBoundary>
        </div>
        <div className="ucbc-zoom">
          <Button title="Zoom out (-)" onClick={() => zoom.current?.zoomBy(1 / 1.25)}>
            −
          </Button>
          <Button title="Fit (0, double-click)" onClick={() => zoom.current?.fit()}>
            Fit
          </Button>
          <Button title="Zoom in (+)" onClick={() => zoom.current?.zoomBy(1.25)}>
            +
          </Button>
        </div>
        {drawError ? <div className="ucbc-error ucbc-draw-error">{drawError}</div> : null}
      </div>
      <div className="ucbc-controls">
        <Button title="First (Home)" onClick={() => seek(0)}>
          ⏮
        </Button>
        <Button title="Previous (←)" onClick={() => seek(frame - 1)}>
          ‹
        </Button>
        <Button title="Play (space)" onClick={toggle}>
          {playing ? "⏸" : "▶"}
        </Button>
        <Button title="Next (→)" onClick={() => seek(frame + 1)}>
          ›
        </Button>
        <Button title="Last (End)" onClick={() => seek(Infinity)}>
          ⏭
        </Button>
        <input
          className="ucbc-slider"
          type="range"
          min={0}
          max={last}
          value={frame}
          onChange={(e) => {
            setPlaying(false);
            seek(Number(e.target.value));
          }}
        />
        <span className="ucbc-position">
          {current.tick ? `tick ${frame} / ${set.ticks.length}` : `start / ${set.ticks.length}`}
        </span>
        <select className="ucbc-speed" value={speed} onChange={(e) => setSpeed(Number(e.target.value))}>
          {SPEEDS.map((s) => (
            <option key={s} value={s}>{`${s}/s`}</option>
          ))}
        </select>
      </div>
      <aside className="ucbc-side">
        <div className="ucbc-info">
          {renderer ? (
            <ErrorBoundary fallback={null} resetKeys={[current.state]}>
              <renderer.Info frame={current} selected={selected} />
            </ErrorBoundary>
          ) : (
            <div className="ucbc-error">{`No renderer for game "${game}".`}</div>
          )}
        </div>
        {selected == null ? null : (
          <BotLog
            bot={selected}
            steps={byBot.get(selected) ?? []}
            frame={frame}
            teams={teams}
            onSeek={seekPaused}
            onClose={() => setSelected(null)}
          />
        )}
        <Steps tick={current.tick} teams={teams} selected={selected} onSelect={setSelected} />
      </aside>
    </div>
  );
}
