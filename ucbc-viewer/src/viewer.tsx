import "./viewer.css";

import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";
import { ErrorBoundary } from "react-error-boundary";

import { ViewerContext, type ViewerState } from "./hooks.ts";
import type { GameRenderer } from "./renderer.ts";
import type { BotId, FailureRecord, Replay, SetReplay, Step, TeamId, TeamInfo, Tick } from "./replay.gen.ts";
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

/** An action's `type`, if it has one, and its other fields as `[key, value]` pairs with
 * strings bare and anything else as JSON. A non-object action is one value under `""`. */
export function actionParts(action: unknown): { type: string | null; fields: [string, string][] } {
  if (typeof action !== "object" || action === null || Array.isArray(action)) {
    return { type: null, fields: [["", JSON.stringify(action)]] };
  }
  const { type, ...rest } = action as Record<string, unknown>;
  const fields = Object.entries(rest).map(([k, v]): [string, string] => [k, typeof v === "string" ? v : JSON.stringify(v)]);
  return { type: type === undefined ? null : String(type), fields };
}

/** An action as its `type`, if it has one, then `key=value` for each other field. */
export function formatAction(action: unknown): string {
  const { type, fields } = actionParts(action);
  const parts = fields.map(([k, v]) => (k ? `${k}=${v}` : v));
  return [...(type === null ? [] : [type]), ...parts].join(" ");
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
  onSelect(bot: BotId): void;
}

/** One step as a table body: a row of numbers, then a row for any output. Clicking it
 * selects the bot. */
function StepRows({ step, teams, onSelect }: StepRowsProps) {
  const memory = step.usage.memory;
  return (
    <tbody className="ucbc-step" onClick={() => onSelect(step.bot)}>
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
                <span key={i} className="ucbc-action">
                  <Action action={a} tokens={NO_TOKENS} />
                </span>
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
  onSelect(bot: BotId): void;
}

function Steps({ tick, teams, onSelect }: StepsProps) {
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
          <StepRows key={i} step={s} teams={teams} onSelect={onSelect} />
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

/** One tick of a bot's timeline: its step, at the frame it produced; or a run of ticks in
 * which it did nothing, from frame `frame` to frame `to`. */
interface Entry {
  frame: number;
  to: number;
  team: TeamId;
  step: Step | null;
}

/** Whether a step did anything worth a node of its own. */
function stepIsIdle(step: Step): boolean {
  return step.actions.length === 0 && !step.stdout && !step.failure;
}

/** Each bot's timeline over a set, in order: an entry per tick it acted or printed in,
 * with runs of idle ticks between them collapsed into one entry each. */
export function stepsByBot(set: SetReplay): Map<BotId, Entry[]> {
  const bots = new Map<BotId, Entry[]>();
  set.ticks.forEach((tick, i) => {
    const frame = i + 1;
    for (const step of tick.steps) {
      let entries = bots.get(step.bot);
      if (!entries) bots.set(step.bot, (entries = []));
      const last = entries.at(-1);
      if (stepIsIdle(step)) {
        if (last && last.step === null) last.to = frame;
        else entries.push({ frame, to: frame, team: step.team, step: null });
      } else {
        entries.push({ frame, to: frame, team: step.team, step });
      }
    }
  });
  return bots;
}

/** A search query as lower-case words. A `key=value` word must equal a whole parameter
 * of an action; any other word may appear anywhere. */
export function searchTokens(query: string): string[] {
  return query.toLowerCase().split(/\s+/).filter(Boolean);
}

const NO_TOKENS: string[] = [];

/** Whether `text` contains every token, ignoring case. Nothing matches no tokens. */
function textMatches(text: string, tokens: string[]): boolean {
  if (!tokens.length) return false;
  const t = text.toLowerCase();
  return tokens.every((tok) => t.includes(tok));
}

/** Whether a parameter chip, or the type, is what a token asks for. */
function partMatches(key: string, value: string, tokens: string[]): boolean {
  const v = value.toLowerCase();
  return tokens.some((tok) => (tok.includes("=") ? `${key.toLowerCase()}=${v}` === tok : v.includes(tok)));
}

/** Whether an action satisfies every token: `key=value` tokens as one of its parameters
 * exactly, others as text of its type or a parameter. */
export function actionMatches(action: unknown, tokens: string[]): boolean {
  if (!tokens.length) return false;
  const { fields } = actionParts(action);
  const text = formatAction(action).toLowerCase();
  return tokens.every((tok) =>
    tok.includes("=") ? fields.some(([k, v]) => `${k.toLowerCase()}=${v.toLowerCase()}` === tok) : text.includes(tok),
  );
}

function failureText(f: FailureRecord): string {
  return `${f.kind}: ${f.message}`;
}

/** Whether anything in an entry matches: one of its actions, its output, or its failure. */
export function entryMatches(entry: Entry, tokens: string[]): boolean {
  const { step } = entry;
  if (!step) return false;
  if (step.actions.some((a) => actionMatches(a, tokens))) return true;
  if (step.stdout && textMatches(step.stdout, tokens)) return true;
  return !!step.failure && textMatches(failureText(step.failure), tokens);
}

/** An action as its type, then a chip per parameter; the parts a search hits are marked. */
function Action({ action, tokens }: { action: unknown; tokens: string[] }) {
  const { type, fields } = actionParts(action);
  const typeHit = type !== null && partMatches("type", type, tokens);
  return (
    <>
      <span className={typeHit ? "ucbc-node-type ucbc-hit" : "ucbc-node-type"}>{type ?? "action"}</span>
      {fields.length ? (
        <span className="ucbc-params">
          {fields.map(([k, v], i) => (
            <code key={i} className={partMatches(k, v, tokens) ? "ucbc-param ucbc-hit" : "ucbc-param"}>
              {k ? <span className="ucbc-param-key">{`${k}=`}</span> : null}
              {v}
            </code>
          ))}
        </span>
      ) : null}
    </>
  );
}

function node(kind: string, hit: boolean): string {
  return hit ? `ucbc-node ucbc-node-${kind} ucbc-match` : `ucbc-node ucbc-node-${kind}`;
}

interface LogEntryProps {
  entry: Entry;
  /** -1 before the frame, 0 at it, 1 after it. */
  when: number;
  /** For an idle run at the frame: the frame, to show where in the run it is. */
  at: number | null;
  tokens: string[];
  onSeek(frame: number): void;
}

/** A tick of the timeline: its number, then the chain of what the bot did in it: each
 * accepted action, then what it printed, then how it failed. Clicking it seeks there. */
const LogEntry = memo(function LogEntry({ entry, when, at, tokens, onSeek }: LogEntryProps) {
  const { step } = entry;
  const cls = when < 0 ? "" : when === 0 ? " ucbc-current" : " ucbc-future";
  const f = step?.failure;
  const run = entry.to - entry.frame + 1;
  const label = run > 1 ? `ticks ${entry.frame}–${entry.to}` : `tick ${entry.frame}`;

  function onClick(e: MouseEvent) {
    // Selecting text to copy is not a click to seek.
    if (window.getSelection()?.isCollapsed === false) return;
    e.preventDefault();
    onSeek(entry.frame);
  }

  return (
    <li className={`ucbc-entry${cls}`} aria-current={when === 0 ? "step" : undefined} onClick={onClick}>
      <button type="button" className="ucbc-link" aria-label={`Seek to ${label}`}>
        {label}
      </button>
      <ol className="ucbc-chain">
        {step ? (
          <>
            {step.actions.map((a, i) => (
              <li key={i} className={node("action", actionMatches(a, tokens))}>
                <Action action={a} tokens={tokens} />
              </li>
            ))}
            {step.stdout ? (
              <li className={node("stdout", textMatches(step.stdout, tokens))}>
                <span className="ucbc-node-type">stdout</span>
                <pre className="ucbc-stdout">{step.stdout}</pre>
              </li>
            ) : null}
            {f ? (
              <li className={node("error", textMatches(failureText(f), tokens))}>
                <span className="ucbc-node-type">{f.kind}</span>
                <div className="ucbc-error">{f.message}</div>
                {f.traceback ? <pre className="ucbc-traceback">{f.traceback}</pre> : null}
              </li>
            ) : null}
          </>
        ) : (
          <li className="ucbc-node ucbc-node-idle">
            <span className="ucbc-node-type">idle</span>
            {at !== null && run > 1 ? <span className="ucbc-muted">{`tick ${at}`}</span> : null}
          </li>
        )}
      </ol>
    </li>
  );
});

interface BotLogProps {
  bot: BotId;
  steps: Entry[];
  frame: number;
  /** 1-based, for messages. */
  setNumber: number;
  playing: boolean;
  teams: TeamInfo[];
  onSeek(frame: number): void;
  onClose(): void;
}

/** A bot's timeline over the set: a linked list of its actions and output, tick by tick.
 * Later ticks are dimmed. While following, the log keeps the latest tick at or before
 * `frame` at its top, so it scrolls along with playback; scrolling it by hand stops that
 * until play, a click on a tick, or the Follow button. A search shows only the ticks with
 * a matching action, parameter, output or failure; Enter seeks to the next match,
 * Shift+Enter to the previous. */
function BotLog({ bot, steps, frame, setNumber, playing, teams, onSeek, onClose }: BotLogProps) {
  const log = useRef<HTMLOListElement>(null);
  // What the log was last scrolled to by the code; any other scroll is the user's.
  const scrolledTo = useRef(-1);
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const tokens = useMemo(() => searchTokens(query), [query]);
  const searching = tokens.length > 0;
  const shown = useMemo(() => (searching ? steps.filter((e) => entryMatches(e, tokens)) : steps), [steps, tokens, searching]);
  let latest = -1;
  for (const [i, e] of shown.entries()) if (e.frame <= frame) latest = i;
  const onMatch = latest >= 0 && shown[latest]!.frame === frame;
  const team = steps[0]?.team;

  useEffect(() => {
    if (playing) setFollow(true);
  }, [playing]);

  useEffect(() => {
    setFollow(true);
  }, [bot]);

  useLayoutEffect(() => {
    const box = log.current;
    const el = box?.children[latest] as HTMLElement | undefined;
    if (!box || !follow) return;
    // The current entry at the top of the box, so the log scrolls along with playback.
    box.scrollTop = el ? el.offsetTop : 0;
    scrolledTo.current = box.scrollTop;
  }, [bot, latest, shown, follow]);

  function onScroll() {
    const box = log.current;
    if (box && box.scrollTop !== scrolledTo.current) setFollow(false);
  }

  // Seeking from the log, by a click or a search jump, follows again.
  const seek = useCallback(
    (to: number) => {
      setFollow(true);
      onSeek(to);
    },
    [onSeek],
  );

  /** Seeks to the first match after the frame, or before it, wrapping around. */
  function jump(forward: boolean) {
    if (!searching || !shown.length) return;
    const before = shown.filter((e) => e.frame < frame);
    const next = forward ? (shown.find((e) => e.frame > frame) ?? shown[0]) : (before.at(-1) ?? shown.at(-1));
    if (next) seek(next.frame);
  }

  function onSearchKey(e: KeyboardEvent<HTMLInputElement>) {
    // The search box keeps its keys from the viewer's shortcuts.
    e.stopPropagation();
    if (e.key === "Enter") jump(!e.shiftKey);
    else if (e.key === "Escape") {
      if (query) setQuery("");
      else onClose();
    }
  }

  const count = !searching
    ? ""
    : shown.length === 0
      ? "no matches"
      : onMatch
        ? `${latest + 1} / ${shown.length}`
        : `${shown.length} ${shown.length === 1 ? "match" : "matches"}`;

  return (
    <section className="ucbc-bot">
      <header className="ucbc-bot-header">
        <span className="ucbc-team">
          {team == null ? null : (
            <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${team}, var(--ucbc-muted))` }} />
          )}
          {team == null ? `Bot ${bot}` : `Bot ${bot} · ${teamName(teams, team)}`}
        </span>
        {follow ? null : (
          <Button title="Scroll along with playback again" onClick={() => setFollow(true)}>
            Follow
          </Button>
        )}
        <Button title="Clear selection (Escape)" onClick={onClose}>
          ×
        </Button>
      </header>
      {steps.length ? (
        <>
          <div className="ucbc-search">
            <input
              type="search"
              className="ucbc-search-input"
              aria-label="Search this bot's actions, output and errors"
              aria-keyshortcuts="Enter Shift+Enter"
              placeholder="Search actions, output… e.g. grab x=11 (/)"
              title="Words match anywhere; key=value matches a whole parameter. Enter: next match, Shift+Enter: previous"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={onSearchKey}
            />
            {searching ? (
              <>
                <span className="ucbc-search-count ucbc-muted" aria-live="polite">
                  {count}
                </span>
                <Button title="Previous match (Shift+Enter)" onClick={() => jump(false)}>
                  ▲
                </Button>
                <Button title="Next match (Enter)" onClick={() => jump(true)}>
                  ▼
                </Button>
              </>
            ) : null}
          </div>
          {shown.length ? (
            <ol className="ucbc-bot-log" ref={log} onScroll={onScroll}>
              {shown.map((e) => {
                const when = frame < e.frame ? 1 : frame > e.to ? -1 : 0;
                const at = when === 0 && !e.step ? frame : null;
                return <LogEntry key={e.frame} entry={e} when={when} at={at} tokens={tokens} onSeek={seek} />;
              })}
            </ol>
          ) : (
            <div className="ucbc-muted">{`No matches in set ${setNumber}. Esc clears the search.`}</div>
          )}
        </>
      ) : (
        <div className="ucbc-muted">{`Bot ${bot} did not play in set ${setNumber}.`}</div>
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

/** Plays a replay. The game's renderer draws the board and its info panel, reading the frame
 * and the selected bot from `ViewerContext`; the viewer owns sets, playback, zoom, the
 * per-tick step table, and the selected bot and its timeline, which replaces the table
 * while a bot is selected. Give it a new `key` to start
 * another replay from the beginning. */
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
  const current = useMemo(() => frameAt(set, teams, frame), [set, teams, frame]);
  // What the game's components read through the hooks.
  const shared = useMemo<ViewerState>(() => ({ frame: current, selected, select: setSelected }), [current, selected]);
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

  // Closing the bot panel unmounts whatever had focus; the viewer takes it back, so the
  // keyboard shortcuts keep working.
  const close = useCallback(() => {
    setSelected(null);
    root.current?.focus({ preventScroll: true });
  }, []);

  function selectSet(index: number) {
    setPlaying(false);
    setSetIndex(index);
    setFrame(0);
    // Bot ids start over in each set.
    close();
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
      Escape: close,
      "/": () => root.current?.querySelector<HTMLInputElement>(".ucbc-search-input")?.focus(),
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
      <ViewerContext value={shared}>
        <header className="ucbc-header">
          <div className="ucbc-title">{teams.map((t) => t.name).join(" vs ")}</div>
          <div className="ucbc-result">
            {`${winner == null ? "Tie" : `${teamName(teams, winner)} wins`} (${score})`}
          </div>
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
                <renderer.Board />
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
                <renderer.Info />
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
              setNumber={setIndex + 1}
              playing={playing}
              teams={teams}
              onSeek={seekPaused}
              onClose={close}
            />
          )}
          {/* The tick's steps give way to the selected bot's timeline. */}
          {selected == null ? (
            <div className="ucbc-steps-box">
              <Steps tick={current.tick} teams={teams} onSelect={setSelected} />
            </div>
          ) : null}
        </aside>
      </ViewerContext>
    </div>
  );
}
