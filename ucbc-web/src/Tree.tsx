import { type FormEvent, useEffect, useState } from "react";
import { Link } from "react-router";
import {
  describe,
  type Api,
  type Match,
  type Submission,
  type User,
} from "./api";
import { MatchBranch, MatchList } from "./matches";

export default function Tree({
  user,
  api,
  onLogOut,
}: {
  user: User;
  api: Api;
  onLogOut: () => void;
}) {
  const [lookup, setLookup] = useState("");
  const [opened, setOpened] = useState<string[]>([]);
  // Bumped after an upload or a queued match so the lists refetch.
  const [version, setVersion] = useState(0);
  const refresh = () => setVersion((v) => v + 1);

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
                <Link to="/profile">{user.display_name}</Link>{" "}
                <span className="dim">{user.is_admin ? "admin" : "user"}</span>
              </li>
              <li>
                <details open>
                  <summary>submissions</summary>
                  <SubmissionsBranch
                    api={api}
                    user={user}
                    version={version}
                    onChange={refresh}
                  />
                </details>
              </li>
              <li>
                <details open>
                  <summary>matches</summary>
                  <ul>
                    <li>
                      <QueueForm
                        api={api}
                        user={user}
                        version={version}
                        onQueued={refresh}
                      />
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
                <Link to="/docs">docs</Link>
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
    api
      .load<Submission[]>("/submissions")
      .then(setAll, (err) => setError(describe(err)));
  }, [version]);

  const mine = all.filter((s) => s.team_id === user.team_id);
  const others = all.filter((s) => s.team_id !== user.team_id);
  const row = (s: Submission) => (
    <li key={s.id}>
      {s.name}{" "}
      <span className="dim">
        {s.game} · {s.team_name} · by {s.display_name} ·{" "}
        {s.created_at.slice(0, 10)} · {s.id}
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
      <input
        type="file"
        accept=".zip"
        onChange={(e) => setFile(e.target.files?.[0] ?? null)}
      />
      <input
        placeholder="name (file name)"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <input
        placeholder="game"
        value={game}
        onChange={(e) => setGame(e.target.value)}
      />
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
    api
      .load<Submission[]>("/submissions")
      .then(setAll, (err) => setError(describe(err)));
  }, [version]);

  const own = all.filter((s) => s.team_id === user.team_id);
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
      {s.name} ({s.team_name}, {s.game})
    </option>
  );
  return (
    <form className="lookup" onSubmit={submit}>
      <select value={mine} onChange={(e) => setMine(e.target.value)}>
        <option value="">our submission</option>
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
