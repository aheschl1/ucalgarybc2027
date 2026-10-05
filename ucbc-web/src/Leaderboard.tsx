import { useEffect, useState } from "react";
import { Link } from "react-router";
import { describe, get, type TeamElo } from "./api";

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
    <main className="page">
      <h1>leaderboard</h1>
      {error && <p className="error">{error}</p>}
      {teams?.length === 0 && <p className="dim">no teams</p>}
      <ol>
        {teams?.map((t) => (
          <li key={t.id} className={t.id === teamId ? "mine" : undefined}>
            {t.name} <span className="dim">· {rating(t)}</span>
          </li>
        ))}
      </ol>

      <p className="back">
        <Link to="/">back</Link>
      </p>
    </main>
  );
}

/** A team no match has moved still sits at the starting rating, which says nothing. */
export function rating(t: TeamElo): string {
  if (t.matches === 0) return "unrated";
  return `elo ${t.elo} · ${t.matches} ${t.matches === 1 ? "match" : "matches"}`;
}
