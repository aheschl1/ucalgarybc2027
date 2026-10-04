import { type FormEvent, useEffect, useState } from "react";
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
}: {
  user: User;
  api: Api;
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

      <section>
        <h2>Submissions</h2>
        <SubmissionsBranch
          api={api}
          user={user}
          version={version}
          onChange={refresh}
        />
      </section>

      <section>
        <h2>Request Match</h2>
          <QueueForm
            api={api}
            user={user}
            version={version}
            onQueued={refresh}
          />

        <h2>Your Matches</h2>
        <ul>
          <MatchList api={api} version={version} mine/>
        </ul>
      </section>

      <section>
        <h2>Match Queue</h2>
        <ul>
          <MatchList api={api} version={version} activeOnly/>
        </ul>
      </section>

      <section>
        <h2>Lookup Match</h2>
        <form className="lookup" onSubmit={open}>
          <input
            placeholder="Search match ID"
            value={lookup}
            onChange={(e) => setLookup(e.target.value)}
          />
          <button disabled={!lookup.trim()}>open</button>
        </form>

        <ul>
          {opened.map((id) => (
            <LookupBranch key={id} id={id} api={api} />
          ))}
        </ul>
      </section>
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
  const [game, setGame] = useState("ucbc2027");
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
        id="file-upload"
        className="file-input"
        type="file"
        accept=".zip"
        onChange={(e) => setFile(e.target.files?.[0] ?? null)}
      />

      <label htmlFor="file-upload" className="file-button">
        Choose File
      </label>

      <input
        placeholder="Name (File name)"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <input
        placeholder="Game"
        value={game}
        onChange={(e) => setGame(e.target.value)}
      />
      <button disabled={!file || !game.trim() || busy}>Upload Zip</button>
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
        bots: [mine, opponent],
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
        <option value="">Submission</option>
        {own.map(option)}
      </select>
      <select value={opponent} onChange={(e) => setOpponent(e.target.value)}>
        <option value="">Opponent</option>
        {all.map(option)}
      </select>
      <input
        type="number"
        title="seed"
        value={seed}
        onChange={(e) => setSeed(Number(e.target.value))}
      />
      <button disabled={!mine || !opponent || busy}>Queue match</button>
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
