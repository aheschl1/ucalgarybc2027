import "./renderer.css";

import { useFrame, useSelection, type GameRenderer, type TeamInfo } from "@ucbc/viewer";

import type { ItemView, UnitEntry } from "./api.gen.ts";
import { Board } from "./board.tsx";
import { useSnapshot } from "./snapshot.ts";

function held(item: ItemView | null | undefined, teams: TeamInfo[]): string {
  if (!item) return "nothing";
  if (item.type === "fossil") return "a fossil";
  return `a level ${item.level} dino of ${teams[item.team]?.name ?? item.team}`;
}

/** The selected unit's details, or that it is gone. */
function Unit({ id, units, teams }: { id: number; units: UnitEntry[]; teams: TeamInfo[] }) {
  const unit = units.find((u) => u.id === id);
  if (!unit) return <div className="ucbc-muted">{`Bot ${id} is not on the board.`}</div>;
  const rows =
    unit.type === "dino"
      ? [
          ["Unit", "dino"],
          ["Level", String(unit.level)],
          ["At", `${unit.pos.x}, ${unit.pos.y}`],
          ["Health", String(unit.health)],
          ["Holds", held(unit.held, teams)],
          ["Vision", String(unit.vision)],
        ]
      : [
          ["Unit", "lab"],
          ["At", `${unit.origin.x}, ${unit.origin.y}`],
          ["Health", String(unit.health)],
          ["Vision", String(unit.vision)],
        ];
  return (
    <table className="ucbc-table u27-unit">
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

function Info() {
  const { set, teams } = useFrame();
  const { selected } = useSelection();
  const s = useSnapshot();
  const first = teams[set.first_team]?.name ?? "";
  const second = teams.find((t) => t.id !== set.first_team)?.name ?? "";
  return (
    <>
      <table className="ucbc-table">
        <thead>
          <tr>
            <th>Team</th>
            <th className="ucbc-num">Bones</th>
            <th className="ucbc-num">Fossils</th>
            <th className="ucbc-num">Dinos</th>
          </tr>
        </thead>
        <tbody>
          {s.teams.map((t, i) => (
            <tr key={i}>
              <td className="ucbc-team">
                <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${i})` }} />
                {teams[i]?.name ?? String(i)}
              </td>
              <td className="ucbc-num">{t.bones}</td>
              <td className="ucbc-num">{t.fossils}</td>
              <td className="ucbc-num">{s.units.filter((u) => u.type === "dino" && u.team === i).length}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="ucbc-muted">{`${first} steps first, then ${second}`}</div>
      {selected == null ? null : <Unit id={selected} units={s.units} teams={teams} />}
    </>
  );
}

/** Draws the board, a table of each team's bones, fossils, and dinos with the set's step
 * order, and the selected unit. Every game package exports its renderer under this name;
 * the viewer's build finds it. */
export const renderer: GameRenderer = { game: "ucbc2027", Board, Info };
