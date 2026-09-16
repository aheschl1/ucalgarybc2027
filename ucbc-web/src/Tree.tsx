import { type FormEvent, useEffect, useState } from "react";
import {
  ApiError,
  get,
  post,
  upload,
  type Match,
  type SetResult,
  type Submission,
  type User,
} from "./api";

type Api = {
  load: <T>(path: string) => Promise<T>;
  post: <T>(path: string, body: unknown) => Promise<T>;
  upload: <T>(path: string, form: FormData) => Promise<T>;
};

export default function Tree({ user, onLogOut }: { user: User; onLogOut: () => void }) {
  const [lookup, setLookup] = useState("");
  const [opened, setOpened] = useState<string[]>([]);
  // Bumped after an upload or a queued match so the lists refetch.
  const [version, setVersion] = useState(0);
  const refresh = () => setVersion((v) => v + 1);

  const guard = async <T,>(call: Promise<T>): Promise<T> => {
    try {
      return await call;
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) onLogOut();
      throw err;
    }
  };
  const api: Api = {
    load: (path) => guard(get(path)),
    post: (path, body) => guard(post(path, body)),
    upload: (path, form) => guard(upload(path, form)),
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
                  <summary>submissions</summary>
                  <SubmissionsBranch api={api} user={user} version={version} onChange={refresh} />
                </details>
              </li>
              <li>
                <details open>
                  <summary>matches</summary>
                  <ul>
                    <li>
                      <QueueForm api={api} user={user} version={version} onQueued={refresh} />
                    </li>
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
                      <LookupBranch key={id} id={id} api={api} />
                    ))}
                    <MatchList api={api} version={version} />
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

function SubmissionsBranch({
  api,
  user,
  version,
  onChange,
}: {
  api: Api;
  user: User;
  version: number;
  onChange: () => void;
}) {
  const [all, setAll] = useState<Submission[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api.load<Submission[]>("/submissions").then(setAll, (err) => setError(describe(err)));
  }, [version]);

  const mine = all.filter((s) => s.user_id === user.id);
  const others = all.filter((s) => s.user_id !== user.id);
  const row = (s: Submission) => (
    <li key={s.id}>
      {s.name}{" "}
      <span className="dim">
        {s.game} · {s.username} · {s.created_at.slice(0, 10)} · {s.id}
      </span>
    </li>
  );

  return (
    <ul>
      <li>
        <UploadForm api={api} onUploaded={onChange} />
      </li>
      {error && (
        <li>
          <span className="error">{error}</span>
        </li>
      )}
      {mine.map(row)}
      {others.length > 0 && (
        <li>
          <details>
            <summary>others</summary>
            <ul>{others.map(row)}</ul>
          </details>
        </li>
      )}
    </ul>
  );
}

function UploadForm({ api, onUploaded }: { api: Api; onUploaded: () => void }) {
  const [file, setFile] = useState<File | null>(null);
  const [name, setName] = useState("");
  const [game, setGame] = useState("tictactoe");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!file) return;
    setBusy(true);
    setError("");
    const form = new FormData();
    form.append("file", file);
    form.append("game", game);
    if (name.trim()) form.append("name", name.trim());
    try {
      await api.upload("/submissions", form);
      setFile(null);
      setName("");
      (e.target as HTMLFormElement).reset();
      onUploaded();
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="lookup" onSubmit={submit}>
      <input type="file" accept=".zip" onChange={(e) => setFile(e.target.files?.[0] ?? null)} />
      <input
        placeholder="name (file name)"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <input placeholder="game" value={game} onChange={(e) => setGame(e.target.value)} />
      <button disabled={!file || !game.trim() || busy}>upload zip</button>
      {error && <span className="error">{error}</span>}
    </form>
  );
}

function QueueForm({
  api,
  user,
  version,
  onQueued,
}: {
  api: Api;
  user: User;
  version: number;
  onQueued: () => void;
}) {
  const [all, setAll] = useState<Submission[]>([]);
  const [mine, setMine] = useState("");
  const [opponent, setOpponent] = useState("");
  const [seed, setSeed] = useState(0);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.load<Submission[]>("/submissions").then(setAll, (err) => setError(describe(err)));
  }, [version]);

  const own = all.filter((s) => s.user_id === user.id);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const a = all.find((s) => s.id === mine);
    if (!a) return;
    setBusy(true);
    setError("");
    try {
      await api.post("/matches/queue", {
        game: a.game,
        bots: [
          { kind: "submission", id: mine },
          { kind: "submission", id: opponent },
        ],
        config: { seed },
      });
      onQueued();
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  };

  const option = (s: Submission) => (
    <option key={s.id} value={s.id}>
      {s.name} ({s.username}, {s.game})
    </option>
  );
  return (
    <form className="lookup" onSubmit={submit}>
      <select value={mine} onChange={(e) => setMine(e.target.value)}>
        <option value="">my submission</option>
        {own.map(option)}
      </select>
      <select value={opponent} onChange={(e) => setOpponent(e.target.value)}>
        <option value="">opponent</option>
        {all.map(option)}
      </select>
      <input
        type="number"
        title="seed"
        value={seed}
        onChange={(e) => setSeed(Number(e.target.value))}
      />
      <button disabled={!mine || !opponent || busy}>queue match</button>
      {error && <span className="error">{error}</span>}
    </form>
  );
}

function MatchList({ api, version }: { api: Api; version: number }) {
  const [rows, setRows] = useState<Match[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api.load<Match[]>("/matches").then(setRows, (err) => setError(describe(err)));
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

function LookupBranch({ id, api }: { id: string; api: Api }) {
  const [row, setRow] = useState<Match | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .load<Match>(`/matches/${encodeURIComponent(id)}`)
      .then(setRow, (err) => setError(describe(err)));
  }, [id]);

  if (error) {
    return (
      <li>
        {id} <span className="error">{error}</span>
      </li>
    );
  }
  if (!row) {
    return (
      <li>
        {id} <span className="dim">loading</span>
      </li>
    );
  }
  return <MatchBranch row={row} api={api} />;
}

function MatchBranch({ row, api }: { row: Match; api: Api }) {
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
          {(match.sets ?? []).map((set) => (
            <SetBranch key={set.index} matchId={match.id} set={set} api={api} />
          ))}
        </ul>
      </details>
    </li>
  );
}

function SetBranch({ matchId, set, api }: { matchId: string; set: SetResult; api: Api }) {
  const [replay, setReplay] = useState<unknown>(null);
  const [error, setError] = useState("");

  const fetchReplay = () => {
    if (replay !== null || error) return;
    api
      .load(`/matches/${encodeURIComponent(matchId)}/sets/${set.index}`)
      .then(setReplay, (err) => setError(describe(err)));
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
