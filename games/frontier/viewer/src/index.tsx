import "./renderer.css";

import { useFrame, useSelection, type GameRenderer } from "@ucbc/viewer";

import type { Points } from "./api.gen.ts";
import { useMemo, type ReactNode } from "react";

import { Board, GLYPHS, MARKS, SHAPES, TERRAIN } from "./board.tsx";
import { Keys, LineChart } from "./chart.tsx";
import { matches, setFind, useFind } from "./find.ts";
import { BUILDINGS, CROPS, timeline, type History, type World } from "./world.ts";

const total = (p: Points | undefined) => (p ? p.buildings + p.stored + p.units : 0);
const TEAM_LINE = ["var(--fr-team-0)", "var(--fr-team-1)"];
const PARTS: (keyof Points)[] = ["buildings", "stored", "units"];

/** Team names, told apart when both teams have the same one. */
function useNames(): string[] {
  const { teams } = useFrame();
  const names = [0, 1].map((t) => teams[t]?.name ?? String(t));
  return names[0] === names[1] ? names.map((n, t) => `${n} (${t + 1})`) : names;
}

function Counts({ w }: { w: World }) {
  const names = useNames();
  const units = [...w.units.values()];
  const structures = [...w.structures.values()];
  return (
    <table className="ucbc-table">
      <thead>
        <tr>
          <th>Team</th>
          <th className="ucbc-num">Points</th>
          <th className="ucbc-num">Units</th>
          <th className="ucbc-num">Buildings</th>
          <th className="ucbc-num">Commander</th>
        </tr>
      </thead>
      <tbody>
        {[0, 1].map((team) => (
          <tr key={team}>
            <td className="ucbc-team">
              <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${team})` }} />
              {names[team]}
            </td>
            <td className="ucbc-num">{total(w.points[team])}</td>
            <td className="ucbc-num">{units.filter((u) => u.team === team).length}</td>
            <td className="ucbc-num">{structures.filter((s) => s.team === team && s.workLeft === 0).length}</td>
            <td className="ucbc-num">{units.find((u) => u.team === team && u.commander)?.hp ?? "dead"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Selected({ id, w }: { id: number; w: World }) {
  const u = w.units.get(id);
  if (!u) return <div className="ucbc-muted">{`Bot ${id} is not on the board: it died, or isn't spawned yet.`}</div>;
  const rows: [string, string][] = [
    ["Unit", u.commander ? "commander" : "worker"],
    ["At", `${u.x}, ${u.y}`],
    ["Health", `${u.hp} / 100`],
    ["Carrying", `${u.carried} of 20 items`],
  ];
  return (
    <table className="ucbc-table fr-detail">
      <tbody>
        {rows.map(([k, v]) => (
          <tr key={k}>
            <th>{k}</th>
            <td>{v}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

const name = (id: string) => id.replace(/_/g, " ");

/** A building or crop: its mark, name, and counts. Clicking searches for it, or clears
 * the search if it already is. */
function Chip({ id, query, mark, counts, title }: { id: string; query: string; mark: ReactNode; counts: ReactNode; title: string }) {
  const on = matches(query, id);
  return (
    <button
      type="button"
      title={title}
      className={`fr-chip${on ? " fr-on" : ""}`}
      onClick={() => setFind(on && query.trim() === name(id) ? "" : name(id))}
    >
      {mark}
      <span className="fr-chip-name">{name(id)}</span>
      {counts}
    </button>
  );
}

/** Every kind of building and crop and how many there are, and a search that rings them
 * on the map. */
function Built({ w, names }: { w: World; names: string[] }) {
  const query = useFind();
  const structures = [...w.structures.values()];
  const crops = [...w.crops.values()];
  return (
    <div className="fr-built">
      <input
        type="search"
        className="fr-search"
        placeholder="Find a building or crop"
        value={query}
        onChange={(e) => setFind(e.target.value)}
      />
      <div className="fr-chips">
        {BUILDINGS.map((b) => {
          const mine = [0, 1].map((t) => structures.filter((s) => s.team === t && s.building === b));
          const done = mine.map((m) => m.filter((s) => s.workLeft === 0).length);
          const title = names.map((n, t) => `${n}: ${done[t]} built, ${mine[t]!.length - done[t]!} being built`).join("\n");
          return (
            <Chip
              key={b}
              id={b}
              query={query}
              title={title}
              mark={<span className={`fr-icon fr-${SHAPES[b]}`}>{GLYPHS[b]}</span>}
              counts={done.map((n, t) => (
                <span key={t} className={`fr-count${n ? "" : " fr-zero"}`} style={{ borderColor: `var(--ucbc-team-${t})` }}>
                  {n}
                </span>
              ))}
            />
          );
        })}
        {CROPS.map((kind) => {
          const all = crops.filter((c) => c.kind === kind);
          const ripe = all.filter((c) => c.ripeAt <= w.tick).length;
          return (
            <Chip
              key={kind}
              id={kind}
              query={query}
              title={`${all.length - ripe} growing, ${ripe} ripe`}
              mark={<span className="ucbc-swatch" style={{ background: MARKS.crop }} />}
              counts={[all.length - ripe, ripe].map((n, i) => (
                <span key={i} className={`fr-count${n ? "" : " fr-zero"}`} style={{ borderColor: i ? MARKS.ripe : MARKS.crop }}>
                  {n}
                </span>
              ))}
            />
          );
        })}
      </div>
    </div>
  );
}

const LEGEND: [string, string, string?][] = [
  ...TERRAIN,
  ["crop", MARKS.crop],
  ["ripe crop", MARKS.ripe],
  ["on the ground", MARKS.pile],
  ["attacked", MARKS.hit],
  ["found", "transparent", MARKS.found],
];

function Legend() {
  return (
    <div className="fr-legend">
      {LEGEND.map(([label, colour, ring]) => (
        <span key={label}>
          <span className="ucbc-swatch" style={{ background: colour, outline: ring && `2px solid ${ring}` }} />
          {label}
        </span>
      ))}
      <span>
        <span className="fr-unit" />
        unit
      </span>
      <span>
        <span className="fr-unit fr-commander" />
        commander
      </span>
    </div>
  );
}

/** Shades of a colour for the sources, darkest at the bottom of the stack. */
const SHADES = [100, 62, 32];

/** Points over the set: each team's total, then where each team's come from. */
function Progress({ history, tick, names, w }: { history: History; tick: number; names: string[]; w: World }) {
  const { totals, parts } = useMemo(() => {
    const totals = [0, 1].map((t) => ({ name: names[t]!, colour: TEAM_LINE[t]!, values: history.points[t]!.map(total) }));
    const parts = [0, 1].map((t) =>
      PARTS.map((key, k) => ({
        name: key,
        colour: `color-mix(in srgb, ${TEAM_LINE[t]} ${SHADES[k]}%, var(--ucbc-panel))`,
        values: history.points[t]!.map((p) => p[key]),
      })),
    );
    return { totals, parts };
  }, [history, names[0], names[1]]);
  const top = Math.max(0, ...totals.flatMap((s) => s.values));
  const finals = totals.map((s) => `${s.name} ${s.values.at(-1) ?? 0}`).join(", ");
  const markers = history.fallen.map((f) => ({ tick: f.tick, label: `${names[f.team]}'s commander fell` }));
  return (
    <div className="fr-progress">
      <h3 className="fr-heading">Points over time</h3>
      <LineChart
        label={`Each team's points over the set, ending at ${finals}`}
        series={totals}
        ticks={history.ticks}
        current={tick}
        markers={markers}
      />
      <Keys entries={totals.map((s) => [s.name, s.colour] as [string, string])} />
      <h3 className="fr-heading">Where points come from</h3>
      <Keys
        areas
        entries={PARTS.map((key, k) => [key, `color-mix(in srgb, var(--ucbc-muted) ${SHADES[k]}%, var(--ucbc-panel))`])}
      />
      {[0, 1].map((t) => (
        <div key={t} className="fr-team-parts">
          <LineChart
            label={`${names[t]}'s points by source over the set`}
            series={parts[t]!}
            ticks={history.ticks}
            current={tick}
            height={80}
            yMax={top}
            stacked
            outline={TEAM_LINE[t]}
          />
          <div className="fr-readout">
            <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${t})` }} />
            {`${names[t]} at tick ${w.tick}: `}
            {PARTS.map((key) => `${w.points[t]?.[key] ?? 0} ${key}`).join(" · ")}
          </div>
        </div>
      ))}
    </div>
  );
}

function Info() {
  const { set, tick } = useFrame();
  const { selected } = useSelection();
  const names = useNames();
  const t = timeline(set);
  const w = t.worldAt(tick ? set.ticks.indexOf(tick) + 1 : 0);
  return (
    <>
      <Counts w={w} />
      {selected == null ? null : <Selected id={selected} w={w} />}
      <Built w={w} names={names} />
      <Legend />
      <Progress history={t.history()} tick={w.tick} names={names} w={w} />
    </>
  );
}

/** Draws the map with its units, buildings, crops, and things on the ground; counts each
 * team's points, units, and buildings, and each kind built; finds buildings and crops;
 * shows the selected unit and a legend. */
export const renderer: GameRenderer = { game: "frontier", Board, Info };
