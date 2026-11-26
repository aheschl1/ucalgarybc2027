import { useEffect, useState } from "react";
import { Link, useParams } from "react-router";
import {
  type Api,
  describe,
  get,
  type Submission,
  type TeamDetails,
} from "./api";
import { MatchList } from "./matches";
import { SubmissionTable } from "./Tree";
import { Card, Count, Empty, ErrorText, Loading, Page, TextLink } from "./ui";

export default function TeamView({ api }: { api: Api | null }) {
  const { teamId } = useParams();
  const [team, setTeam] = useState<TeamDetails | null>(null);
  const [bots, setBots] = useState<Submission[] | null>(null);
  const [error, setError] = useState("");
  const [botsError, setBotsError] = useState("");

  useEffect(() => {
    if (!teamId) return;

    let current = true;
    setTeam(null);
    setBots(null);
    setError("");
    setBotsError("");
    get<TeamDetails>(`/teams/${encodeURIComponent(teamId)}`).then(
      (loaded) => {
        if (!current) return;
        setTeam(loaded);
        api?.load<Submission[]>(`/submissions?team=${loaded.id}`).then(
          (rows) => current && setBots(rows),
          (err) => current && setBotsError(describe(err)),
        );
      },
      (err) => current && setError(describe(err)),
    );
    return () => {
      current = false;
    };
  }, [teamId, api]);

  return (
    <Page title={team?.name ?? "Team"}>
      <div className="flex flex-col gap-6">
        <p>
          <Link to="/leaderboard" className="text-accent hover:underline">
            ← Leaderboard
          </Link>
        </p>

        {error && <ErrorText>{error}</ErrorText>}
        {!team && !error && <Loading rows={3} />}

        {team && (
          <>
            <Card title="Team information">
              <div className="grid gap-5 sm:grid-cols-3">
                <div>
                  <p className="mb-1 text-xs font-medium text-muted">Rating</p>
                  <p>{team.matches === 0 ? "Unrated" : `${team.elo} elo`}</p>
                </div>
                <div>
                  <p className="mb-1 text-xs font-medium text-muted">Matches</p>
                  <p>{team.matches}</p>
                </div>
                <div>
                  <p className="mb-2 text-xs font-medium text-muted">
                    Members <Count n={team.members.length} />
                  </p>
                  {team.members.length > 0 ? (
                    <ul className="flex flex-wrap gap-1.5">
                      {team.members.map((member, i) => (
                        <li
                          key={i}
                          className="flex items-center gap-1.5 rounded-full border border-line bg-panel py-0.5 pr-2.5 pl-0.5"
                        >
                          <span className="grid size-5 place-items-center rounded-full bg-hover text-[10px] font-semibold uppercase">
                            {member.slice(0, 1)}
                          </span>
                          {member}
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p>No members</p>
                  )}
                </div>
              </div>
            </Card>

            {!api ? (
              <Card title="Bots and matches">
                <p>
                  <TextLink to={`/login?next=/teams/${team.id}`}>
                    Log in
                  </TextLink>{" "}
                  to see this team's bots and match history.
                </p>
              </Card>
            ) : (
              <>
                <Card
                  title={<>Bots {bots && <Count n={bots.length} />}</>}
                  flush
                >
                  {botsError ? (
                    <div className="p-4">
                      <ErrorText>{botsError}</ErrorText>
                    </div>
                  ) : !bots ? (
                    <Loading />
                  ) : bots.length === 0 ? (
                    <Empty>No bots yet</Empty>
                  ) : (
                    <SubmissionTable rows={bots} />
                  )}
                </Card>

                <Card title="Match history" flush>
                  <MatchList
                    api={api}
                    version={0}
                    query={{ team: team.id }}
                    empty="No matches yet"
                    hint="Platform matches, and any against your team, appear here once played."
                  />
                </Card>
              </>
            )}
          </>
        )}
      </div>
    </Page>
  );
}
