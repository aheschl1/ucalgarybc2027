import { type FormEvent, type MouseEvent, useEffect, useState } from "react";
import { Link } from "react-router";
import {
  describe,
  type Api,
  type Submission,
  type Team,
  type TeamElo,
  type User,
} from "./api";
import { rating } from "./Leaderboard";
import { MatchList } from "./matches";

export default function Profile({
  user,
  api,
  onMove,
}: {
  user: User;
  api: Api;
  onMove: (team: Team) => void;
}) {
  const [bots, setBots] = useState<Submission[]>([]);
  const [error, setError] = useState("");
  // Bumped after a move to another team so the team's lists refetch.
  const [version, setVersion] = useState(0);

  useEffect(() => {
    api
      .load<Submission[]>("/submissions?mine=true")
      .then(setBots, (err) => setError(describe(err)));
  }, [version]);

  const moved = (team: Team) => {
    onMove(team);
    setVersion((v) => v + 1);
  };

  return (
    <main className="page">
      <h1>{user.display_name}</h1>
      <p className="dim">
        {user.email} · {user.is_admin ? "Admin" : "User"} · Joined{" "}
        {user.created_at.slice(0, 10)}
      </p>

      <hr />

      <TeamSection api={api} onMove={moved} />

      <h2>Bots</h2>
      {error ? (
        <p className="error">{error}</p>
      ) : (
        bots.length === 0 && <p className="dim">none</p>
      )}
      <ul>
        {bots.map((s) => (
          <li key={s.id}>
            {s.name}{" "}
            <span className="dim">
              {s.game} · by {s.display_name} · {s.created_at.slice(0, 10)} ·{" "}
              {s.id}
            </span>
          </li>
        ))}
      </ul>

      <h2>Matches</h2>
      <ul>
        <MatchList api={api} version={version} mine />
      </ul>
    </main>
  );
}

function TeamSection({
  api,
  onMove,
}: {
  api: Api;
  onMove: (team: Team) => void;
}) {
  const [team, setTeam] = useState<Team | null>(null);
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.load<Team>("/teams/me").then(setTeam, (err) => setError(describe(err)));
  }, []);

  // The whole board, for the team's place on it; refetched when the team changes.
  const [board, setBoard] = useState<TeamElo[]>([]);
  useEffect(() => {
    if (team) api.load<TeamElo[]>("/teams/elo").then(setBoard, () => {});
  }, [team?.id]);
  const mine = board.find((t) => t.id === team?.id);

  const act = async (call: () => Promise<Team>, moves: boolean) => {
    setBusy(true);
    setError("");
    try {
      const next = await call();
      setTeam(next);
      setCode("");
      setName("");
      if (moves) onMove(next);
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  };

  // Only members see the code, so the last one out leaves nobody able to join.
  const leave = () =>
    !team ||
    team.members.length > 1 ||
    window.confirm(
      `You are the last member of ${team.name}. Once you leave nobody can join it, ` +
        "and its bots stay with it.",
    );

  const join = (e: FormEvent) => {
    e.preventDefault();
    if (leave())
      act(() => api.post<Team>("/teams/join", { code: code.trim() }), true);
  };
  const start = (e: FormEvent) => {
    e.preventDefault();
    if (leave())
      act(() => api.post<Team>("/teams", { name: name.trim() }), true);
  };
  const newCode = (e: MouseEvent) => {
    e.preventDefault();
    act(() => api.post<Team>("/teams/me/code", undefined), false);
  };

  return (
    <>
      {team && (
        <>
          <h2>Team {team ? team.name : "team"}</h2>

          <p>Members: {team.members.join(", ")}</p>
          {mine && (
            <p className="dim">
              {rating(mine)} · #{board.indexOf(mine) + 1} of {board.length} ·{" "}
              <Link to="/leaderboard">Leaderboard</Link>
            </p>
          )}
          <p className="dim">
            Join code: <code>{team.join_code}</code> ·{" "}
            <a href="#" onClick={newCode}>
              Generate New Code
            </a>
          </p>
        </>
      )}
      <ul>
        <li>
          <form className="lookup" onSubmit={join}>
            <input
              placeholder="Enter Team Join Code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
            />
            <button disabled={busy || !code.trim()}>Join Team</button>
          </form>
        </li>
        <li>
          <form className="lookup" onSubmit={start}>
            <input
              placeholder="Enter New Team Name"
              maxLength={64}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <button disabled={busy || !name.trim()}>Start Team</button>
          </form>
        </li>
      </ul>
      {error && <p className="error">{error}</p>}
      <p className="dim">
        Joining or starting a team moves you out of this one; its bots and
        matches stay with it.
      </p>
    </>
  );
}
