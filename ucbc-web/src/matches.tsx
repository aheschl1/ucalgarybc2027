import { useEffect, useState } from "react";
import { Link } from "react-router";
import { describe, type Api, type Match, type SetResult } from "./api";

export function MatchList({
  api,
  version,
  mine,
}: {
  api: Api;
  version: number;
  mine?: boolean;
}) {
  const [rows, setRows] = useState<Match[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .load<Match[]>(mine ? "/matches?mine=true" : "/matches")
      .then(setRows, (err) => setError(describe(err)));
  }, [version]);

  if (error) {
    return (
      <li>
        <span className="error">{error}</span>
      </li>
    );
  }
  return (
    <>
      {rows.map((row) => (
        <MatchBranch key={row.id} row={row} api={api} />
      ))}
    </>
  );
}

export function MatchBranch({ row, api }: { row: Match; api: Api }) {
  // The list gives the row without its sets; those load when the branch opens.
  const [match, setMatch] = useState<Match>(row);
  const [error, setError] = useState("");
  useEffect(() => setMatch(row), [row]);

  const fetchSets = () => {
    if (match.sets || error) return;
    api
      .load<Match>(`/matches/${encodeURIComponent(row.id)}`)
      .then(setMatch, (err) => setError(describe(err)));
  };

  const teams = match.teams.map((t) => t.name).join(" vs ");
  const wins = match.set_wins ? ` · set wins ${match.set_wins.join("–")}` : "";
  const origin = match.origin === "platform" ? " · platform" : "";
  return (
    <li>
      <details onToggle={(e) => e.currentTarget.open && fetchSets()}>
        <summary>
          {match.id}{" "}
          <span className="dim">
            {match.status} · {match.game} · {teams}
            {wins}
            {origin}
          </span>
          {match.error && <span className="error"> {match.error}</span>}
        </summary>
        <ul>
          {error && (
            <li>
              <span className="error">{error}</span>
            </li>
          )}
          {match.status === "done" && (
            <li>
              <Link to={`/viewer/${encodeURIComponent(match.id)}`}>watch</Link>
            </li>
          )}
          {(match.sets ?? []).map((set) => (
            <SetLine key={set.index} set={set} />
          ))}
        </ul>
      </details>
    </li>
  );
}

function SetLine({ set }: { set: SetResult }) {
  const winner =
    set.winner_team === null ? "no winner" : `winner ${set.winner_team}`;
  return (
    <li>
      set {set.index}{" "}
      <span className="dim">
        {winner} · {set.reason}
        {set.detail && ` (${set.detail})`} · {set.ticks} ticks
      </span>
    </li>
  );
}
