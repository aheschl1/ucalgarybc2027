import "./renderer.css";

import { useFrame, useSelection, type GameRenderer } from "@ucbc/viewer";

import type { Points } from "./api.gen.ts";
import { useMemo } from "react";

import { Board, MARKS, TERRAIN } from "./board.tsx";
import { Keys, LineChart } from "./chart.tsx";
import { timeline, type History, type World } from "./world.ts";

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

const LEGEND: [string, string][] = [
  ...TERRAIN,
  ["crop", MARKS.crop],
  ["ripe crop", MARKS.ripe],
  ["things on the ground", MARKS.pile],
  ["attacked this tick", MARKS.hit],
];

function Legend() {
  return (
    <div className="fr-legend">
      {LEGEND.map(([name, colour]) => (
        <span key={name}>
          <span className="ucbc-swatch" style={{ background: colour }} />
          {name}
        </span>
      ))}
      <span>
        Units are small squares, buildings larger; a ringed unit is a commander, a white centre a
        base, a dark square with a coloured edge a trap, which the other team can't see.
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
      <Progress history={t.history()} tick={w.tick} names={names} w={w} />
      <Legend />
    </>
  );
}

/** Draws the map with its units, buildings, crops, and things on the ground; counts each
 * team's points, units, and buildings; shows the selected unit and a legend. */
export const renderer: GameRenderer = { game: "frontier", Board, Info };
