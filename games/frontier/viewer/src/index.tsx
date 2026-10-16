import "./renderer.css";

import { useFrame, useSelection, type GameRenderer } from "@ucbc/viewer";

import type { Points } from "./api.gen.ts";
import { useEffect, useMemo, type ReactNode } from "react";

import { Board, GLYPHS, MARKS, SHAPES, TERRAIN } from "./board.tsx";
import { Keys, LineChart } from "./chart.tsx";
import { found, name, setFind, useFind } from "./find.ts";
import { picked } from "./store.ts";
import { BUILDINGS, CROPS, timeline, type History, type Stack, type World } from "./world.ts";

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
          <th className="ucbc-num" title="The commander's health">Cmdr</th>
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

/** Items as "12 wood, 3 stone", or "nothing". */
function Stacks({ stacks }: { stacks: Stack[] }) {
  if (!stacks.length) return <span className="ucbc-muted">nothing</span>;
  return <>{stacks.map(([item, n]) => `${n} ${name(item)}`).join(", ")}</>;
}

/** A unit or building: a line saying what and where it is, then what it holds. */
function Detail({ head, facts, rows }: { head: ReactNode; facts: string[]; rows: [string, ReactNode][] }) {
  return (
    <div className="fr-detail">
      <div className="fr-detail-head">
        {head}
        <span className="ucbc-muted">{facts.join(" · ")}</span>
      </div>
      <table className="ucbc-table">
        <tbody>
          {rows.map(([k, v]) => (
            <tr key={k}>
              <th>{k}</th>
              <td>{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Selected({ id, w, names }: { id: number; w: World; names: string[] }) {
  const u = w.units.get(id);
  if (!u) return <div className="ucbc-muted">{`Bot ${id} is not on the board: it died, or isn't spawned yet.`}</div>;
  const items = w.carrying.get(id) ?? [];
  return (
    <Detail
      head={
        <>
          <span className="fr-unit" style={{ background: `var(--ucbc-team-${u.team})` }} />
          <strong>{u.commander ? "commander" : "worker"}</strong>
        </>
      }
      facts={[names[u.team]!, `${u.x}, ${u.y}`, `${u.hp} HP`]}
      rows={[
        // Replays from before contents were recorded have only the count.
        ["Carrying", <>{`${u.carried} of 20`}{items.length ? <>{": "}<Stacks stacks={items} /></> : null}</>],
      ]}
    />
  );
}

function Picked({ id, w, names }: { id: number; w: World; names: string[] }) {
  const s = w.structures.get(id);
  if (!s) return <div className="ucbc-muted">That building is gone.</div>;
  const held = w.contents.get(id) ?? { stored: [], owed: [] };
  const room = w.storage[BUILDINGS.indexOf(s.building)] ?? 0;
  const total = held.stored.reduce((n, [, c]) => n + c, 0);
  const rows: [string, ReactNode][] = [];
  if (s.workLeft > 0) rows.push(["Being built", <>{`${s.workLeft} work left; owed `}<Stacks stacks={held.owed} /></>]);
  else if (room) rows.push(["Storage", <>{`${total} of ${room}: `}<Stacks stacks={held.stored} /></>]);
  if (held.job) {
    const left = held.job.doneAt - w.tick;
    rows.push(["Making", `${name(held.job.recipe)}, ${left > 0 ? `ready in ${left} ticks` : "ready"}`]);
  }
  return (
    <Detail
      head={
        <>
          <span className={`fr-icon fr-${SHAPES[s.building]}`}>{GLYPHS[s.building]}</span>
          <strong>{name(s.building)}</strong>
        </>
      }
      facts={[names[s.team]!, `${s.x}, ${s.y}`, `${s.hp} HP`]}
      rows={rows}
    />
  );
}

/** A building or crop: its mark, name, and counts. Clicking searches for it, or clears
 * the search if it already is. */
function Chip({ id, on, mark, counts, label }: { id: string; on: boolean; mark: ReactNode; counts: ReactNode; label: string }) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={on}
      className={`fr-chip${on ? " fr-on" : ""}`}
      onClick={() => setFind(on ? "" : name(id))}
    >
      <span aria-hidden className="fr-chip-face">
        {mark}
        <span className="fr-chip-name">{name(id)}</span>
        {counts}
      </span>
    </button>
  );
}

/** Every kind of building and crop and how many there are, and a search that rings them
 * on the map. */
function Built({ w, names }: { w: World; names: string[] }) {
  const { set } = useFrame();
  const query = useFind();
  useEffect(() => {
    setFind("");
    picked.set(null);
  }, [set]);
  const kinds = found(query);
  // Per kind and team: finished, then being built.
  const built = new Map<string, [number, number][]>(BUILDINGS.map((b) => [b, [[0, 0], [0, 0]]]));
  for (const s of w.structures.values()) built.get(s.building)![s.team]![s.workLeft > 0 ? 1 : 0]!++;
  // Per kind: growing, then ripe.
  const grown = new Map<string, [number, number]>(CROPS.map((c) => [c, [0, 0]]));
  for (const c of w.crops.values()) grown.get(c.kind)![c.ripeAt <= w.tick ? 1 : 0]!++;
  const total = (id: string) => (built.get(id)?.flat() ?? grown.get(id)!).reduce((a, b) => a + b, 0);
  const count = [...kinds].reduce((n, id) => n + total(id), 0);
  return (
    <div className="fr-built">
      <div className="fr-search-row">
        <input
          type="search"
          className="fr-search"
          placeholder="Find a building or crop"
          aria-label="Find a building or crop"
          value={query}
          onChange={(e) => setFind(e.target.value)}
        />
        {query.trim() ? <span className="ucbc-muted">{kinds.size ? `${count} found` : "no matches"}</span> : null}
      </div>
      <div className="fr-chips">
        {BUILDINGS.filter((b) => total(b) || kinds.has(b)).map((b) => {
          const teams = built.get(b)!;
          return (
            <Chip
              key={b}
              id={b}
              on={kinds.has(b)}
              label={`${name(b)}: ${teams.map(([done, more], t) => `${names[t]} ${done} built, ${more} being built`).join("; ")}`}
              mark={<span className={`fr-icon fr-${SHAPES[b]}`}>{GLYPHS[b]}</span>}
              counts={teams.map(([done, more], t) => (
                <span key={t} className={`fr-count${done + more ? "" : " fr-zero"}`} style={{ borderColor: `var(--ucbc-team-${t})` }}>
                  {done}
                  {more ? <span className="ucbc-muted">+{more}</span> : null}
                </span>
              ))}
            />
          );
        })}
        {CROPS.filter((c) => total(c) || kinds.has(c)).map((c) => {
          const [growing, ripe] = grown.get(c)!;
          return (
            <Chip
              key={c}
              id={c}
              on={kinds.has(c)}
              label={`${name(c)}: ${growing} growing, ${ripe} ripe`}
              mark={<span className="fr-crop" />}
              counts={
                <span>
                  {growing + ripe}
                  {ripe ? <span className="ucbc-muted">{` · ${ripe} ripe`}</span> : null}
                </span>
              }
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
          <span className="fr-swatch" style={{ background: colour, outline: ring && `2px solid ${ring}` }} />
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
  const building = picked.use();
  const names = useNames();
  const t = timeline(set);
  const w = t.worldAt(tick ? set.ticks.indexOf(tick) + 1 : 0);
  return (
    <>
      <Counts w={w} />
      {selected != null ? <Selected id={selected} w={w} names={names} /> : building != null ? <Picked id={building} w={w} names={names} /> : null}
      <Built w={w} names={names} />
      <Progress history={t.history()} tick={w.tick} names={names} w={w} />
      <Legend />
    </>
  );
}

/** Draws the map with its units, buildings, crops, and things on the ground; counts each
 * team's points, units, and buildings, and each kind built; finds buildings and crops;
 * shows the selected unit and a legend. */
export const renderer: GameRenderer = { game: "frontier", Board, Info };
