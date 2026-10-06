import { useEffect, useState } from "react";
import { describe, get, type TeamElo } from "./api";
import { Card, cx, Empty, ErrorText, Page, Table } from "./ui";

/** Every team's rating, highest first. Public; `teamId` marks the viewer's own team. */
export default function Leaderboard({ teamId }: { teamId?: number }) {
  const [teams, setTeams] = useState<TeamElo[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    get<TeamElo[]>("/teams/elo").then(setTeams, (err) =>
      setError(describe(err)),
    );
  }, []);

  return (
    <Page title="Leaderboard">
      <Card flush>
        {error && (
          <div className="p-4">
            <ErrorText>{error}</ErrorText>
          </div>
        )}
        {teams?.length === 0 && <Empty>No teams</Empty>}
        {teams && teams.length > 0 && (
          <Table head={["#", "Team", "Elo", "Matches"]}>
            {teams.map((t, i) => {
              const mine = t.id === teamId;
              return (
                <tr key={t.id} className={cx(mine && "bg-accent-soft")}>
                  <td
                    className={cx(
                      "w-12 border-l-2 px-4 py-3 font-mono text-xs tabular-nums",
                      mine ? "border-accent" : "border-transparent",
                      i < 3 ? "font-semibold" : "text-muted",
                    )}
                  >
                    {i + 1}
                  </td>
                  <td className="px-4 py-3 font-medium">
                    {t.name}
                    {mine && <span className="ml-2 text-xs font-normal text-accent">your team</span>}
                  </td>
                  <td className="px-4 py-3 font-mono tabular-nums">
                    {t.matches === 0 ? <span className="text-muted">unrated</span> : t.elo}
                  </td>
                  <td className="px-4 py-3 text-muted tabular-nums">{t.matches}</td>
                </tr>
              );
            })}
          </Table>
        )}
      </Card>
    </Page>
  );
}

/** A team no match has moved still sits at the starting rating, which says nothing. */
export function rating(t: TeamElo): string {
  if (t.matches === 0) return "unrated";
  return `elo ${t.elo} · ${t.matches} ${t.matches === 1 ? "match" : "matches"}`;
}
