import "./renderer.css";

import { useFrame, useSelection, type GameRenderer } from "@ucbc/viewer";

import { Board, MARKS, TERRAIN } from "./board.tsx";
import { timeline, type World } from "./world.ts";

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
            <td className="ucbc-num">{w.points[team] ?? ""}</td>
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

function Info() {
  const { set, tick } = useFrame();
  const { selected } = useSelection();
  const w = timeline(set).worldAt(tick ? set.ticks.indexOf(tick) + 1 : 0);
  return (
    <>
      <Counts w={w} />
      {selected == null ? null : <Selected id={selected} w={w} />}
      <Legend />
    </>
  );
}

/** Draws the map with its units, buildings, crops, and things on the ground; counts each
 * team's points, units, and buildings; shows the selected unit and a legend. */
export const renderer: GameRenderer = { game: "frontier", Board, Info };
