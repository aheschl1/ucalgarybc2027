import { useEffect, useState } from "react";
import { describe, get, type TeamElo } from "./api";
import { Card, cx, Empty, ErrorText, Loading, Page, Table } from "./ui";

/** Every team's rating, highest first. Public; `teamId` marks the viewer's own team. */
export default function Leaderboard({ teamId }: { teamId?: number }) {
  const [teams, setTeams] = useState<TeamElo[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    get<TeamElo[]>("/teams/elo").then(setTeams, (err) =>
      setError(describe(err)),
    );
  }, []);

  // Bars span the rated teams' range, so small gaps in rating still read as gaps.
  const rated = teams?.filter((t) => t.matches > 0).map((t) => t.elo) ?? [];
  const lo = Math.min(...rated) - 50;
  const hi = Math.max(...rated);

  return (
    <Page title="Leaderboard" subtitle="Elo from ranked matches, updated as each one finishes.">
      <Card flush>
        {error && (
          <div className="p-4">
            <ErrorText>{error}</ErrorText>
          </div>
        )}
        {!teams && !error && <Loading rows={6} />}
        {teams?.length === 0 && <Empty hint="Teams appear here once they exist.">No teams yet</Empty>}
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
                      i === 0 ? "font-semibold text-accent" : i < 3 ? "font-semibold" : "text-muted",
                    )}
                  >
                    {i + 1}
                  </td>
                  <td className="px-4 py-3 font-medium">
                    {t.name}
                    {mine && <span className="ml-2 text-xs font-normal text-accent">your team</span>}
                  </td>
                  <td className="w-1/3 px-4 py-3">
                    {t.matches === 0 ? (
                      <span className="text-muted">unrated</span>
                    ) : (
                      <div className="flex items-center gap-3">
                        <span className="w-10 font-mono tabular-nums">{t.elo}</span>
                        <span className="hidden h-1 flex-1 overflow-hidden rounded-full bg-hover sm:block">
                          <span
                            className={cx("block h-full rounded-full", mine ? "bg-accent" : "bg-fg/70")}
                            style={{ width: `${((t.elo - lo) / Math.max(1, hi - lo)) * 100}%` }}
                          />
                        </span>
                      </div>
                    )}
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
