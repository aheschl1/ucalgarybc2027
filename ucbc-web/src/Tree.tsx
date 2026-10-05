import { type FormEvent, useEffect, useState } from "react";
import {
  describe,
  type Api,
  type GameMap,
  type Match,
  type Submission,
  type User,
} from "./api";
import { MatchBranch, MatchList, MatchNote } from "./matches";
import {
  Button,
  Card,
  cx,
  day,
  Empty,
  ErrorText,
  Field,
  Input,
  Mono,
  Page,
  Select,
  Table,
} from "./ui";

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
    <Page wide>
      <div className="grid gap-6 lg:grid-cols-2">
        <Card title="Submit a bot">
          <UploadForm api={api} onUploaded={refresh} />
        </Card>
        <Card title="Request a match">
          <QueueForm api={api} user={user} version={version} onQueued={refresh} />
        </Card>

        <Card
          className="lg:col-span-2"
          title="Your matches"
          flush
          action={
            <form className="flex w-full gap-2 sm:w-auto" onSubmit={open}>
              <Input
                className="sm:w-64"
                placeholder="Match ID"
                value={lookup}
                onChange={(e) => setLookup(e.target.value)}
              />
              <Button variant="secondary" disabled={!lookup.trim()}>
                Open
              </Button>
            </form>
          }
        >
          <MatchList
            api={api}
            version={version}
            mine
            empty="No matches"
            before={opened.map((id) => (
              <LookupBranch key={id} id={id} api={api} />
            ))}
          />
        </Card>

        <Card className="lg:col-span-2" title="Match queue" flush>
          <MatchList
            api={api}
            version={version}
            activeOnly
            empty="Empty"
          />
        </Card>

        <Card className="lg:col-span-2" title="Submissions" flush>
          <SubmissionsBranch api={api} user={user} version={version} />
        </Card>
      </div>
    </Page>
  );
}

function SubmissionsBranch({
  api,
  user,
  version,
}: {
  api: Api;
  user: User;
  version: number;
}) {
  const [all, setAll] = useState<Submission[]>([]);
  const [error, setError] = useState("");
  const [showOthers, setShowOthers] = useState(false);

  useEffect(() => {
    api
      .load<Submission[]>("/submissions")
      .then(setAll, (err) => setError(describe(err)));
  }, [version]);

  const mine = all.filter((s) => s.team_id === user.team_id);
  const others = all.filter((s) => s.team_id !== user.team_id);

  if (error) {
    return (
      <div className="p-4">
        <ErrorText>{error}</ErrorText>
      </div>
    );
  }
  return (
    <>
      {mine.length > 0 ? (
        <SubmissionTable rows={mine} />
      ) : (
        <Empty>No submissions</Empty>
      )}
      {others.length > 0 && (
        <div className="border-t border-line">
          <button
            className="flex w-full cursor-pointer items-center gap-2 px-4 py-3 text-left text-muted hover:text-fg"
            onClick={() => setShowOthers(!showOthers)}
          >
            <svg
              viewBox="0 0 16 16"
              className={cx("size-3.5 transition-transform", showOthers && "rotate-90")}
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M6 4l4 4-4 4" />
            </svg>
            Other teams ({others.length})
          </button>
          {showOthers && <SubmissionTable rows={others} team />}
        </div>
      )}
    </>
  );
}

/** Submissions, newest as the API sends them. `team` adds the owning team's column. */
export function SubmissionTable({ rows, team }: { rows: Submission[]; team?: boolean }) {
  return (
    <Table head={["Name", "Game", ...(team ? ["Team"] : []), "By", "Uploaded", "ID"]}>
      {rows.map((s) => (
        <tr key={s.id}>
          <td className="px-4 py-3 font-medium">{s.name}</td>
          <td className="px-4 py-3 text-muted">{s.game}</td>
          {team && <td className="px-4 py-3">{s.team_name}</td>}
          <td className="px-4 py-3 text-muted">{s.display_name}</td>
          <td className="px-4 py-3 whitespace-nowrap text-muted tabular-nums">
            {day(s.created_at)}
          </td>
          <td className="px-4 py-3">
            <Mono value={s.id} />
          </td>
        </tr>
      ))}
    </Table>
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
    <form className="flex flex-col gap-4" onSubmit={submit}>
      <label
        className={cx(
          "flex cursor-pointer flex-col items-center justify-center gap-1 rounded-lg border border-dashed px-4 py-6 text-center transition-colors hover:border-fg",
          file ? "border-fg bg-panel" : "border-line",
        )}
      >
        <input
          className="sr-only"
          type="file"
          accept=".zip"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
        />
        <span className="font-medium">{file ? file.name : "Choose a .zip"}</span>
        <span className="text-xs text-muted">
          {file ? `${(file.size / 1024).toFixed(1)} KiB` : "Max 1 MiB"}
        </span>
      </label>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Name">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <Field label="Game">
          <Input value={game} onChange={(e) => setGame(e.target.value)} />
        </Field>
      </div>
      <div className="flex items-center gap-3">
        <Button variant="accent" disabled={!file || !game.trim() || busy}>
          {busy ? "Uploading…" : "Upload"}
        </Button>
        {error && <ErrorText>{error}</ErrorText>}
      </div>
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
  const [maps, setMaps] = useState<GameMap[]>([]);
  const [mine, setMine] = useState("");
  const [opponent, setOpponent] = useState("");
  const [seed, setSeed] = useState(0);
  const [sets, setSets] = useState(3);
  // A map id per set; "" is the game's standard map.
  const [picked, setPicked] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .load<Submission[]>("/submissions")
      .then(setAll, (err) => setError(describe(err)));
    api
      .load<GameMap[]>("/maps")
      .then(setMaps, (err) => setError(describe(err)));
  }, [version]);

  // Admins may queue any submissions; a member needs one of their team's.
  const own = user.is_admin
    ? all
    : all.filter((s) => s.team_id === user.team_id);
  // The match's game is the first picked bot's, and the maps offered are that game's.
  const game = (
    all.find((s) => s.id === mine) ?? all.find((s) => s.id === opponent)
  )?.game;
  // Another game's maps do not carry over.
  useEffect(() => setPicked([]), [game]);
  const offered = maps.filter((m) => m.game === game && m.archived_at === null);
  const chosen = Array.from({ length: sets }, (_, i) => picked[i] ?? "");
  // The standard map everywhere, or a map in every set; the API takes no mix.
  const partial = chosen.some(Boolean) && !chosen.every(Boolean);
  const pick = (set: number, id: string) => {
    const next = [...chosen];
    next[set] = id;
    setPicked(next);
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!game || partial) return;
    setBusy(true);
    setError("");
    try {
      await api.post("/matches/queue", {
        game,
        bots: [mine, opponent],
        config: { seed, sets },
        // One map for every set when they are all the same.
        maps:
          new Set(chosen).size === 1 && chosen[0]
            ? [chosen[0]]
            : chosen.filter(Boolean),
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
    <form className="flex flex-col gap-4" onSubmit={submit}>
      <Field label={user.is_admin ? "Submission" : "Your submission"}>
        <Select value={mine} onChange={(e) => setMine(e.target.value)}>
          <option value="">Choose…</option>
          {own.map(option)}
        </Select>
      </Field>
      <div className="grid grid-cols-[1fr_7rem_5rem] gap-3">
        <Field label="Opponent">
          <Select value={opponent} onChange={(e) => setOpponent(e.target.value)}>
            <option value="">Choose…</option>
            {all.map(option)}
          </Select>
        </Field>
        <Field label="Seed">
          <Input
            type="number"
            value={seed}
            onChange={(e) => setSeed(Number(e.target.value))}
          />
        </Field>
        <Field label="Sets">
          <Input
            type="number"
            min={1}
            value={sets}
            onChange={(e) => setSets(Math.max(1, Number(e.target.value)))}
          />
        </Field>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {chosen.map((id, i) => (
          <Field key={i} label={`Set ${i + 1} map`}>
            <Select
              title={game ? undefined : "Choose a submission to pick maps"}
              disabled={!game}
              value={id}
              onChange={(e) => pick(i, e.target.value)}
            >
              <option value="">Standard map</option>
              {offered.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </Select>
          </Field>
        ))}
      </div>
      <div className="flex items-center gap-3">
        <Button disabled={!mine || !opponent || partial || busy}>
          {busy ? "Queueing…" : "Queue match"}
        </Button>
        {partial && <ErrorText>Pick a map for every set, or none</ErrorText>}
        {error && <ErrorText>{error}</ErrorText>}
      </div>
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
      <MatchNote id={id}>
        <span className="text-accent">{error}</span>
      </MatchNote>
    );
  }
  if (!row) return <MatchNote id={id}>…</MatchNote>;
  return <MatchBranch row={row} api={api} />;
}
