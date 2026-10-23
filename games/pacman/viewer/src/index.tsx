import "./renderer.css";

import { useFrame, useSelection, type GameRenderer } from "@ucbc/viewer";

import type { AgentView } from "./api.gen.ts";
import { Board } from "./board.tsx";
import { useSnapshot } from "./snapshot.ts";

function Rows({ rows, className = "" }: { rows: [string, string][]; className?: string }) {
  return (
    <table className={`ucbc-table ${className}`}>
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

/** The selected agent's details, or that it is gone. */
function Agent({ id, agents }: { id: number; agents: AgentView[] }) {
  const agent = agents.find((a) => a.id === id);
  if (!agent) return <div className="ucbc-muted">{`Bot ${id} is not on the board.`}</div>;
  const ghost = agents.filter((a) => a.role === "ghost").indexOf(agent);
  return (
    <Rows
      className="pm-detail"
      rows={[
        ["Agent", agent.role === "pacman" ? "Pac-Man" : `ghost ${ghost + 1}`],
        ["At", `${agent.pos.x}, ${agent.pos.y}`],
        ...(agent.role === "ghost"
          ? [["Frightened", agent.frightened ? `${agent.frightened} ticks` : "no"] as [string, string]]
          : []),
      ]}
    />
  );
}

function Info() {
  const { teams } = useFrame();
  const { selected } = useSelection();
  const s = useSnapshot();
  const name = (team: number) => teams[team]?.name ?? String(team);
  const ghosts = s.agents.filter((a) => a.role === "ghost").length;
  return (
    <>
      <table className="ucbc-table">
        <thead>
          <tr>
            <th>Plays</th>
            <th>Team</th>
          </tr>
        </thead>
        <tbody>
          {[
            ["Pac-Man", s.pacman_team],
            [`${ghosts} ghost${ghosts === 1 ? "" : "s"}`, s.ghost_team],
          ].map(([role, team]) => (
            <tr key={team}>
              <td>{role}</td>
              <td className="ucbc-team">
                <span className="ucbc-swatch" style={{ background: `var(--ucbc-team-${team})` }} />
                {name(team as number)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Rows
        rows={[
          ["Pellets left", String(s.pellets_left)],
          ["Ghosts eaten", String(s.ghosts_eaten)],
        ]}
      />
      {selected == null ? null : <Agent id={selected} agents={s.agents} />}
    </>
  );
}

/** Draws the maze and agents, which team plays which side, the pellets left and ghosts
 * eaten, and the selected agent. Every game package exports its renderer under this name;
 * the viewer's build finds it. */
export const renderer: GameRenderer = { game: "pacman", Board, Info };
