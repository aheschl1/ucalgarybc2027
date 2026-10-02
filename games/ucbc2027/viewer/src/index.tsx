import "./state.css";

import type { BoardProps, FrameProps, GameRenderer, TeamInfo } from "@ucbc/viewer";

import type { Coord, ItemView, Snapshot, UnitEntry } from "./api.gen.ts";

function snapshot(state: unknown): Snapshot {
  const s = state as Snapshot;
  if (typeof s?.tick !== "number" || !Array.isArray(s.environment)) {
    throw new Error(`not a ucbc2027 state: ${JSON.stringify(state)}`);
  }
  return s;
}

/** The tiles a lab with top-left corner `origin` covers, as `lab_footprint` in map.rs. */
function footprint({ x, y }: Coord): Coord[] {
  return [
    { x, y },
    { x: x + 1, y },
    { x, y: y + 1 },
    { x: x + 1, y: y + 1 },
  ];
}

/** Clicking a dino or a lab's tile selects that bot; any other tile clears the selection. */
function Board({ frame, selected, onSelect }: BoardProps) {
  const s = snapshot(frame.state);
  const width = s.environment[0]?.length ?? 0;
  const tiles = s.environment.flat().map((env) => ({
    classes: ["u27-tile", `u27-${env.type}`, ...(env.type === "lab" ? [`u27-team-${env.team}`] : [])],
    text: "",
    bot: undefined as number | undefined,
  }));
  const at = ({ x, y }: Coord) => tiles[y * width + x];
  for (const fossil of s.fossils) {
    const tile = at(fossil);
    if (tile) {
      tile.classes.push("u27-fossil");
      tile.text = "◆";
    }
  }
  for (const unit of s.units) {
    if (unit.type !== "lab") continue;
    for (const tile of footprint(unit.origin).map(at)) {
      if (!tile) continue;
      tile.bot = unit.id;
      if (unit.id === selected) tile.classes.push("u27-selected");
    }
  }
  for (const unit of s.units) {
    if (unit.type !== "dino") continue;
    const tile = at(unit.pos);
    if (!tile) continue;
    tile.classes.push("u27-dino", `u27-team-${unit.team}`);
    if (unit.held) tile.classes.push("u27-held");
    if (unit.id === selected) tile.classes.push("u27-selected");
    tile.text = String(unit.level);
    tile.bot = unit.id;
  }
  return (
    <div
      className="u27-board"
      style={{ gridTemplateColumns: `repeat(${width}, auto)` }}
      onClick={(e) => {
        const tile = (e.target as Element).closest<HTMLElement>("[data-bot]");
        onSelect(tile ? Number(tile.dataset.bot) : null);
      }}
    >
      {tiles.map((t, i) => (
        <span key={i} className={t.classes.join(" ")} data-bot={t.bot}>
          {t.text}
        </span>
      ))}
    </div>
  );
}

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
        ]
      : [
          ["Unit", "lab"],
          ["At", `${unit.origin.x}, ${unit.origin.y}`],
          ["Health", String(unit.health)],
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

function Info({ frame: { state, set, teams }, selected }: FrameProps) {
  const s = snapshot(state);
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
