import { type FormEvent, useEffect, useState } from "react";
import { ApiError, get, type Match, type SetResult } from "./api";
import type { Session } from "./App";

type Load = <T>(path: string) => Promise<T>;

export default function Tree({ session, onLogOut }: { session: Session; onLogOut: () => void }) {
  const { token, user } = session;
  const [lookup, setLookup] = useState("");
  const [opened, setOpened] = useState<string[]>([]);

  const load: Load = async (path) => {
    try {
      return await get(path, token);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) onLogOut();
      throw err;
    }
  };

  const open = (e: FormEvent) => {
    e.preventDefault();
    const id = lookup.trim();
    if (id && !opened.includes(id)) setOpened([id, ...opened]);
    setLookup("");
  };

  return (
    <main className="tree">
      <ul>
        <li>
          <details open>
            <summary>ucbc</summary>
            <ul>
              <li>
                <details>
                  <summary>account</summary>
                  <ul>
                    <li>{user.username}</li>
                    <li className="dim">{user.is_admin ? "admin" : "user"}</li>
                    <li className="dim">created {user.created_at.slice(0, 10)}</li>
                  </ul>
                </details>
              </li>
              <li>
                <details open>
                  <summary>matches</summary>
                  <ul>
                    <li>
                      <form className="lookup" onSubmit={open}>
                        <input
                          placeholder="match id"
                          value={lookup}
                          onChange={(e) => setLookup(e.target.value)}
                        />
                        <button disabled={!lookup.trim()}>open</button>
                      </form>
                    </li>
                    {opened.map((id) => (
                      <MatchBranch key={id} id={id} load={load} />
                    ))}
                  </ul>
                </details>
              </li>
              <li>
                <a
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    onLogOut();
                  }}
                >
                  log out
                </a>
              </li>
            </ul>
          </details>
        </li>
      </ul>
    </main>
  );
}

function MatchBranch({ id, load }: { id: string; load: Load }) {
  const [match, setMatch] = useState<Match | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    load<Match>(`/matches/${encodeURIComponent(id)}`).then(setMatch, (err) => setError(describe(err)));
  }, [id]);

  if (error) {
    return (
      <li>
        {id} <span className="error">{error}</span>
      </li>
    );
  }
  if (!match) {
    return (
      <li>
        {id} <span className="dim">loading</span>
      </li>
    );
  }

  const teams = match.teams.map((t) => t.name).join(" vs ");
  const wins = match.set_wins ? ` · set wins ${match.set_wins.join("–")}` : "";
  return (
    <li>
      <details>
        <summary>
          {id}{" "}
          <span className="dim">
            {match.status} · {match.game} · {teams}
            {wins}
          </span>
        </summary>
        <ul>
          {match.sets.map((set) => (
            <SetBranch key={set.index} matchId={id} set={set} load={load} />
          ))}
        </ul>
      </details>
    </li>
  );
}

function SetBranch({ matchId, set, load }: { matchId: string; set: SetResult; load: Load }) {
  const [replay, setReplay] = useState<unknown>(null);
  const [error, setError] = useState("");

  const fetchReplay = () => {
    if (replay !== null || error) return;
    load(`/matches/${encodeURIComponent(matchId)}/sets/${set.index}`).then(setReplay, (err) =>
      setError(describe(err)),
    );
  };

  const winner = set.winner_team === null ? "no winner" : `winner ${set.winner_team}`;
  return (
    <li>
      <details>
        <summary>
          set {set.index}{" "}
          <span className="dim">
            {winner} · {set.reason}
            {set.detail && ` (${set.detail})`} · {set.ticks} ticks
          </span>
        </summary>
        <ul>
          <li>
            <details onToggle={(e) => e.currentTarget.open && fetchReplay()}>
              <summary>replay</summary>
              {error ? (
                <p className="error">{error}</p>
              ) : replay === null ? (
                <p className="dim">loading</p>
              ) : (
                <pre>{JSON.stringify(replay, null, 2)}</pre>
              )}
            </details>
          </li>
        </ul>
      </details>
    </li>
  );
}

function describe(err: unknown): string {
  return err instanceof ApiError ? `${err.status} ${err.message}` : "could not reach the api";
}
