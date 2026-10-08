import { type ReactNode, useEffect, useState } from "react";
import { Link } from "react-router";
import {
  describe,
  type Api,
  type GameMap,
  type Match,
  type MatchRow,
  type SetResult,
} from "./api";
import { Chevron, cx, Empty, ErrorText, Loading, Mono, StatusPill, Table, Time } from "./ui";

const HEAD = ["", "Match", "Score", "Status", "Queued", ""];
const COLUMNS = HEAD.length;

/** Filters for `GET /matches`; each one narrows the list. */
export type MatchQuery = {
  mine?: boolean;
  active?: boolean;
  origin?: MatchRow["origin"];
  team?: number;
};

// How often a list with a match still to play asks again.
const POLL_MS = 3000;

const live = (m: MatchRow) => m.status === "queued" || m.status === "running";

/** Matches as a table, narrowed by `query`; `before` puts rows of the caller's own on top.
 * While any is still to play it refetches on its own; `onLoad` hears each fetch. */
export function MatchList({
  api,
  version,
  query = {},
  before,
  empty = "No matches",
  hint,
  onLoad,
}: {
  api: Api;
  version: number;
  query?: MatchQuery;
  before?: ReactNode;
  empty?: string;
  hint?: ReactNode;
  onLoad?: (rows: MatchRow[]) => void;
}) {
  const [rows, setRows] = useState<MatchRow[] | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== false) params.set(key, String(value));
  }
  const search = params.toString();

  useEffect(() => {
    // A filter changed before the last list came back; that list is dropped.
    let current = true;
    setError("");
    api.load<MatchRow[]>(search ? `/matches?${search}` : "/matches").then(
      (r) => {
        if (!current) return;
        setRows(r);
        onLoad?.(r);
      },
      (err) => current && setError(describe(err)),
    );
    return () => {
      current = false;
    };
  }, [version, search, tick]);

  useEffect(() => {
    if (!rows?.some(live)) return;
    const timer = setTimeout(() => setTick((t) => t + 1), POLL_MS);
    return () => clearTimeout(timer);
  }, [rows]);

  if (error) {
    return (
      <div className="p-4">
        <ErrorText>{error}</ErrorText>
      </div>
    );
  }
  if (!rows && !before) return <Loading />;
  if (rows?.length === 0 && !before) return <Empty hint={hint}>{empty}</Empty>;
  return (
    <Table head={HEAD}>
      {before}
      {rows?.map((row) => <MatchBranch key={row.id} row={row} api={api} />)}
    </Table>
  );
}

/** A row standing in for a match that has not loaded, or failed to. */
export function MatchNote({ id, children }: { id: string; children: ReactNode }) {
  return (
    <tr>
      <td />
      <td className="px-4 py-3">
        <Mono value={id} />
      </td>
      <td colSpan={COLUMNS - 2} className="px-4 py-3 text-muted">
        {children}
      </td>
    </tr>
  );
}

export function MatchBranch({ row, api }: { row: MatchRow | Match; api: Api }) {
  const [match, setMatch] = useState<MatchRow | Match>(row);
  const [open, setOpen] = useState(false);
  const [mapNames, setMapNames] = useState<string[] | null>(null);
  const [error, setError] = useState("");
  // A list refetching hands a new row; the sets already loaded hold while the status does.
  useEffect(
    () => setMatch((prev) => ("sets" in prev && prev.status === row.status ? prev : row)),
    [row],
  );

  // The list gives the row without its sets; those load when the branch opens.
  useEffect(() => {
    if (!open || "sets" in match || error) return;
    api
      .load<Match>(`/matches/${encodeURIComponent(row.id)}`)
      .then(setMatch, (err) => setError(describe(err)));
  }, [open, match]);
  // The row names its maps by id; the names come from the map list.
  const fetchMapNames = () => {
    if (mapNames || row.maps.length === 0) return;
    api.load<GameMap[]>("/maps").then(
      (maps) => {
        const names = new Map(maps.map((m) => [m.id, m.name]));
        setMapNames(row.maps.map((id) => names.get(id) ?? id));
      },
      (err) => setError(describe(err)),
    );
  };
  const toggle = () => {
    if (!open) fetchMapNames();
    setOpen(!open);
  };

  return (
    <>
      <tr
        onClick={toggle}
        className={cx("cursor-pointer transition-colors hover:bg-hover", open && "bg-panel")}
      >
        <td className="w-8 pl-4">
          <Chevron open={open} />
        </td>
        <td className="max-w-80 px-4 py-3">
          <Teams match={match} />
          <div className="mt-0.5 flex items-center gap-2 text-xs text-muted">
            {match.game}
            {match.origin === "platform" && (
              <span className="rounded border border-line px-1.5 text-[11px]">ranked</span>
            )}
            <Mono value={match.id} />
          </div>
        </td>
        <td className="px-4 py-3 font-mono whitespace-nowrap tabular-nums">
          {match.set_wins ? match.set_wins.join(" – ") : <span className="text-muted">–</span>}
        </td>
        <td className="px-4 py-3">
          <StatusPill status={match.status} />
        </td>
        <td className="px-4 py-3 text-muted">
          <Time iso={match.created_at} />
        </td>
        <td className="px-4 py-3 text-right">
          {match.status === "done" && (
            <Link
              to={`/platform/matches/${encodeURIComponent(match.id)}`}
              onClick={(e) => e.stopPropagation()}
              className="inline-flex items-center gap-1.5 rounded-md border border-line bg-bg px-2.5 py-1 text-xs font-medium transition hover:border-fg"
            >
              <svg viewBox="0 0 16 16" className="size-3" fill="currentColor" aria-hidden>
                <path d="M5 3.5v9l7-4.5z" />
              </svg>
              Watch
            </Link>
          )}
        </td>
      </tr>
      {open && (
        <tr className="bg-panel">
          <td />
          <td colSpan={COLUMNS - 1} className="px-4 pb-4">
            {match.error && <ErrorText>{match.error}</ErrorText>}
            {error && <ErrorText>{error}</ErrorText>}
            <p className="mb-2 text-xs text-muted">
              {match.maps.length === 0
                ? "Standard map"
                : `${match.maps.length === 1 ? "Map" : "Maps"} ${(mapNames ?? match.maps).join(", ")}`}
            </p>
            {"sets" in match ? (
              match.sets.length > 0 ? (
                <ul className="divide-y divide-line rounded-lg border border-line bg-bg">
                  {match.sets.map((set) => (
                    <SetLine key={set.index} set={set} match={match} />
                  ))}
                </ul>
              ) : (
                !match.error && (
                  <p className="text-muted">
                    {match.status === "done" ? "No sets" : "Sets show here once it finishes."}
                  </p>
                )
              )
            ) : (
              !error && <p className="animate-pulse text-muted">Loading sets…</p>
            )}
          </td>
        </tr>
      )}
    </>
  );
}

/** "A vs B", with the winner in bold. */
function Teams({ match }: { match: MatchRow }) {
  return (
    <div className="truncate">
      {match.teams.map((t, i) => (
        <span key={t.id}>
          {i > 0 && <span className="text-muted"> vs </span>}
          <span className={match.winner_team === t.id ? "font-semibold" : undefined}>
            {t.name}
          </span>
        </span>
      ))}
    </div>
  );
}

function SetLine({ set, match }: { set: SetResult; match: MatchRow }) {
  const winner =
    set.winner_team === null
      ? "No winner"
      : (match.teams.find((t) => t.id === set.winner_team)?.name ??
        `Team ${set.winner_team}`);
  return (
    <li className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 px-3 py-2">
      <span className="w-12 text-xs text-muted">Set {set.index}</span>
      <span className="font-medium">{winner}</span>
      <span className="text-muted">
        {set.reason}
        {set.detail && ` (${set.detail})`}
      </span>
      <span className="ml-auto font-mono text-xs text-muted tabular-nums">
        {set.ticks} ticks
      </span>
    </li>
  );
}
