import { useEffect, useState } from "react";
import { Link } from "react-router";
import { describe, type Api, type Submission, type User } from "./api";
import { MatchList } from "./matches";

export default function Profile({ user, api }: { user: User; api: Api }) {
  const [bots, setBots] = useState<Submission[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .load<Submission[]>("/submissions?mine=true")
      .then(setBots, (err) => setError(describe(err)));
  }, []);

  return (
    <main className="page">
      <h1>{user.display_name}</h1>
      <p className="dim">
        {user.email} · {user.is_admin ? "admin" : "user"} · joined{" "}
        {user.created_at.slice(0, 10)}
      </p>

      <h2>bots</h2>
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
              {s.game} · {s.created_at.slice(0, 10)} · {s.id}
            </span>
          </li>
        ))}
      </ul>

      <h2>matches</h2>
      <ul>
        <MatchList api={api} version={0} mine />
      </ul>

      <p className="back">
        <Link to="/">back</Link>
      </p>
    </main>
  );
}
