import { type FormEvent, useEffect, useState } from "react";
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
import { SubmissionTable } from "./Tree";
import { Button, Card, Count, day, Empty, ErrorText, Field, Input, Page, useCopy } from "./ui";

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
    <Page>
      <div className="flex flex-col gap-6">
        <div className="flex items-center gap-4">
          <span className="grid size-14 shrink-0 place-items-center rounded-full bg-fg text-xl font-semibold text-bg uppercase">
            {user.display_name.slice(0, 1)}
          </span>
          <div className="min-w-0">
            <h1 className="truncate text-2xl font-semibold tracking-tight">{user.display_name}</h1>
            <p className="truncate text-muted">
              {user.email} · {user.is_admin ? "Admin" : "Member"} · Joined{" "}
              {day(user.created_at)}
            </p>
          </div>
        </div>

        <TeamSection api={api} onMove={moved} />

        <Card title={<>Your bots <Count n={bots.length} /></>} flush>
          {error ? (
            <div className="p-4">
              <ErrorText>{error}</ErrorText>
            </div>
          ) : bots.length === 0 ? (
            <Empty hint="Upload one from the platform page.">No bots yet</Empty>
          ) : (
            <SubmissionTable rows={bots} />
          )}
        </Card>

        <Card title="Matches" flush>
          <MatchList
            api={api}
            version={version}
            query={{ mine: true }}
            empty="No matches yet"
            hint="Your team's matches show here once queued."
          />
        </Card>
      </div>
    </Page>
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
  const [copied, copy] = useCopy();

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
  const newCode = () =>
    act(() => api.post<Team>("/teams/me/code", undefined), false);

  return (
    <>
      {team && (
        <Card
          title={team.name}
          action={
            mine && (
              <Link
                to="/leaderboard"
                className="text-sm text-muted hover:text-fg hover:underline"
              >
                #{board.indexOf(mine) + 1} of {board.length} · {rating(mine)}
              </Link>
            )
          }
        >
          <div className="flex flex-col gap-5">
            <div>
              <p className="mb-2 text-xs font-medium text-muted">Members</p>
              <ul className="flex flex-wrap gap-1.5">
                {team.members.map((m) => (
                  <li
                    key={m}
                    className="flex items-center gap-1.5 rounded-full border border-line bg-panel py-0.5 pr-2.5 pl-0.5"
                  >
                    <span className="grid size-5 place-items-center rounded-full bg-hover text-[10px] font-semibold uppercase">
                      {m.slice(0, 1)}
                    </span>
                    {m}
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <p className="mb-2 text-xs font-medium text-muted">
                Join code <span className="font-normal">· share it with teammates</span>
              </p>
              <div className="flex flex-wrap items-center gap-2">
                <code className="h-9 rounded-lg border border-dashed border-muted/50 bg-panel px-3 font-mono text-base leading-9 tracking-[0.2em]">
                  {team.join_code}
                </code>
                <Button variant="secondary" type="button" onClick={() => copy(team.join_code)}>
                  {copied ? "Copied" : "Copy"}
                </Button>
                <Button variant="ghost" type="button" disabled={busy} onClick={newCode}>
                  New code
                </Button>
              </div>
            </div>
          </div>
        </Card>
      )}

      <Card title="Switch team">
        <div className="grid gap-4 sm:grid-cols-2">
          <form className="grid grid-cols-[1fr_auto] items-end gap-2" onSubmit={join}>
            <Field label="Join with a code">
              <Input value={code} onChange={(e) => setCode(e.target.value)} />
            </Field>
            <Button variant="secondary" disabled={busy || !code.trim()}>
              Join
            </Button>
          </form>
          <form className="grid grid-cols-[1fr_auto] items-end gap-2" onSubmit={start}>
            <Field label="Start a new team">
              <Input
                placeholder="Team name"
                maxLength={64}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </Field>
            <Button variant="secondary" disabled={busy || !name.trim()}>
              Start
            </Button>
          </form>
        </div>
        {error && (
          <div className="mt-3">
            <ErrorText>{error}</ErrorText>
          </div>
        )}
      </Card>
    </>
  );
}
