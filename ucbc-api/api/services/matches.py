from datetime import datetime, timedelta
import gzip
from uuid import UUID

from api.blobs import BlobStore
from api.db import DBConnection
from api.errors import ApiError, Forbidden, NotFound
from api.models.matches import (
    Match,
    MatchEnqueue,
    MatchOrigin,
    MatchReplay,
    MatchResult,
    MatchRow,
    TeamInfo,
)
from api.models.users import User
from api.services import elo
from api.services.submissions import get_submission

LIST_LIMIT = 50


class LostLease(Exception):
    """Another worker took the match over; nothing was written."""

    def __init__(self, match: MatchRow) -> None:
        super().__init__(f"match {match.id} is no longer held")


async def enqueue_match(db: DBConnection, user: User, req: MatchEnqueue) -> UUID:
    """Admins queue any submissions at any priority. A member needs one of their team's
    submissions in the match, at normal priority. Anyone picks the maps; they are not read
    here, so a bad pick fails the match when it is played."""
    names: list[str] = []
    owned = False
    for bot in req.bots:
        submission = await get_submission(db, bot)
        if submission.game != req.game:
            raise ApiError(f"submission {bot} is for {submission.game}, not {req.game}")
        owned = owned or submission.team_id == user.team_id
        names.append(submission.name)
    if not user.is_admin and not owned:
        raise Forbidden("one of the bots must be your team's submission")
    priority = req.priority if user.is_admin else 0
    teams = [TeamInfo(id=i, name=name) for i, name in enumerate(names)]
    return await db.match_repo.insert(
        "user", req.game, teams, req.config, req.bots, req.maps, priority
    )


async def get_match(db: DBConnection, user: User, match_id: UUID) -> Match:
    row = await _visible(db, user, match_id)
    sets = await db.match_repo.list_set_results(match_id)
    return Match(**row.model_dump(), sets=sets)


async def get_set_replay(db: DBConnection, user: User, match_id: UUID, index: int, blobs: BlobStore) -> bytes:
    """The set's replay as stored: JSON, gzipped."""
    await _visible(db, user, match_id)
    replay = await db.match_repo.get_set_replay(match_id, index)
    if replay is None:
        raise NotFound(f"no set {index} in match {match_id}")

    replay_key = replay
    if replay_key is not None:
        return await blobs.get(replay_key)

    return replay


async def list_matches(
    db: DBConnection,
    user: User,
    mine: bool = False,
    origin: MatchOrigin | None = None,
    team: int | None = None,
    active: bool = False,
) -> list[MatchRow]:
    """Platform matches and matches with one of the caller's team's submissions, newest
    first; every match for an admin. `mine` narrows to the team's own, admin or not; `team`
    to another team's, of those the caller may see; `origin` to user or platform matches;
    `active` to matches queued or running."""
    return await db.match_repo.list_recent(
        user.team_id if mine or not user.is_admin else None,
        LIST_LIMIT,
        owned_only=mine,
        origin=origin,
        involving=team,
        active=active,
    )


async def _visible(db: DBConnection, user: User, match_id: UUID) -> MatchRow:
    """A platform match or one with the caller's team's submission, or any for an admin;
    otherwise as if missing."""
    row = await db.match_repo.get(match_id, None if user.is_admin else user.team_id)
    if row is None:
        raise NotFound(f"no match {match_id}")
    return row


# The worker side of the queue.


async def claim_match(db: DBConnection, worker: str, lease: timedelta) -> MatchRow | None:
    """The next match to play, or None when the queue is empty. A match claimed more than
    `max_attempts` times is failed here instead of handed out."""
    while (match := await db.match_repo.claim(worker, lease)) is not None:
        if match.attempts <= match.max_attempts:
            return match
        await fail_match(db, match, f"gave up after {match.attempts - 1} attempts")
    return None


async def finish_match(
    db: DBConnection, match: MatchRow, replay: MatchReplay, blobs: BlobStore
) -> None:
    """Records the replay, marks the match done, and moves both teams' ratings. Sets from
    an earlier attempt are replaced, so a rerun after a lost worker leaves one consistent
    replay, and a lost lease writes nothing."""
    replay_keys: dict[int, str] = {}
    for s in replay.sets:
        key = f"matches/{match.id}/attempts/{match.attempts}/sets/{s.index}.json.gz"
        data = gzip.compress(s.model_dump_json(exclude_unset=True).encode(), 6) # please check this
        await blobs.put(key, data, "application/json")
        replay_keys[s.index] = key

    async with db.conn.transaction():
        await db.match_repo.delete_sets(match.id)
        for s in replay.sets:
            await db.match_repo.add_set(
                match.id, s, replay_keys[s.index] # i'm pretty sure it's s.index
            )
        done = await db.match_repo.complete(
            match.id,
            _claimed_at(match),
            replay.result.set_wins,
            replay.result.winner_team,
            replay.engine_version,
        )
        if not done:
            raise LostLease(match)
        await _rate(db, match, replay.result)


async def _rate(db: DBConnection, match: MatchRow, result: MatchResult) -> None:
    """Steps both slots' teams by slot 0's share of the sets. A team in both slots plays
    itself, which rates nothing."""
    teams = []
    for bot in match.bots:
        submission = await db.submission_repo.get(bot)
        if submission is None:
            raise ValueError(f"match {match.id}: no submission {bot}")
        teams.append(submission.team_id)
    a, b = teams
    if a == b:
        return
    elos = await db.team_repo.lock_elos([a, b])
    new_a, new_b = elo.step(elos[a], elos[b], elo.score(result.set_wins, len(result.sets)))
    await db.team_repo.record_elo(a, new_a)
    await db.team_repo.record_elo(b, new_b)


async def fail_match(db: DBConnection, match: MatchRow, error: str) -> None:
    if not await db.match_repo.fail(match.id, _claimed_at(match), error):
        raise LostLease(match)


async def requeue_match(db: DBConnection, match: MatchRow) -> None:
    """After a failure to start the match; a lost lease is fine, the match already moved on."""
    await db.match_repo.requeue(match.id, _claimed_at(match))


async def heartbeat(db: DBConnection, match: MatchRow) -> bool:
    return await db.match_repo.heartbeat(match.id, _claimed_at(match))


def _claimed_at(match: MatchRow) -> datetime:
    if match.claimed_at is None:
        raise ValueError(f"match {match.id} is not claimed")
    return match.claimed_at
