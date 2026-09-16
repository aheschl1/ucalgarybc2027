import { createViewer, renderers, type Replay, type SetReplay } from "@ucbc/viewer";
import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router";
import { describe, type Api, type Match } from "./api";

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
  const stage = useRef<HTMLDivElement>(null);
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
          (match.sets ?? []).map((s) =>
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

  useEffect(() => {
    if (!replay || !stage.current) return;
    const viewer = createViewer(stage.current, { renderers, replay });
    return () => viewer.destroy();
  }, [replay]);

  return (
    <main className="viewer">
      <p className="dim">
        <Link to="/">ucbc</Link> · match {matchId}
      </p>
      {error ? (
        <p className="error">{error}</p>
      ) : (
        !replay && <p className="dim">{status}</p>
      )}
      <div ref={stage} />
    </main>
  );
}
