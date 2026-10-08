import {
  renderers,
  Viewer as ReplayViewer,
  type Replay,
  type SetReplay,
} from "@ucbc/viewer";
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router";
import { describe, type Api, type Match } from "./api";
import { ErrorText, Mono } from "./ui";

/** The replay file the engine would have written, from the match row and its sets. */
function assemble(match: Match, sets: SetReplay[]): Replay {
  const c = match.config;
  return {
    match_id: match.id,
    engine_version: match.engine_version ?? "",
    config: {
      game: match.game,
      sets: c.sets,
      seed: c.seed,
      teams: match.teams.length,
      limits: { step_ms: c.step_ms, memory_bytes: c.memory_bytes },
    },
    teams: match.teams,
    sets,
    result: {
      sets: sets.map((s) => s.result),
      set_wins: match.set_wins ?? [],
      winner_team: match.winner_team,
    },
  };
}

/** Plays one match. A match that is not done yet shows its status instead. */
export default function Viewer({ api }: { api: Api }) {
  const { matchId = "" } = useParams();
  const [replay, setReplay] = useState<Replay | null>(null);
  const [status, setStatus] = useState("loading");
  const [error, setError] = useState("");

  useEffect(() => {
    // The effect runs twice under StrictMode; only the live run may set state.
    let live = true;
    setReplay(null);
    setStatus("loading");
    setError("");
    const id = encodeURIComponent(matchId);
    api
      .load<Match>(`/matches/${id}`)
      .then(async (match) => {
        if (match.status !== "done") {
          return match.error ? `${match.status}: ${match.error}` : match.status;
        }
        const sets = await Promise.all(
          match.sets.map((s) =>
            api.load<SetReplay>(`/matches/${id}/sets/${s.index}`),
          ),
        );
        if (live) setReplay(assemble(match, sets));
        return "";
      })
      .then(
        (text) => live && setStatus(text),
        (err) => live && setError(describe(err)),
      );
    return () => {
      live = false;
    };
  }, [matchId]);

  return (
    <main className="mx-auto w-full max-w-[84rem] px-4 py-6 sm:px-6">
      <p className="mb-4 flex items-center gap-2 text-sm text-muted">
        <Link to="/platform" className="hover:text-fg">
          Platform
        </Link>
        <span>/</span>
        <span>Match</span>
        <Mono value={matchId} short={36} />
      </p>
      {error ? (
        <ErrorText>{error}</ErrorText>
      ) : (
        !replay && (
          <p className="py-16 text-center text-muted">
            {status === "loading" ? "…" : status}
          </p>
        )
      )}
      {replay && <ReplayViewer replay={replay} renderers={renderers} />}
    </main>
  );
}
