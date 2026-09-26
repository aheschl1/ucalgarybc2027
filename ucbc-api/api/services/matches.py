from datetime import datetime, timedelta
from uuid import UUID

from api.db import DBConnection
from api.errors import ApiError, Forbidden, NotFound
from api.models.matches import (
    Match,
    MatchEnqueue,
    MatchReplay,
    MatchRow,
    PathSource,
    SetReplay,
    TeamInfo,
)
from api.models.users import User
from api.services.submissions import get_submission

LIST_LIMIT = 50


class LostLease(Exception):
    """Another worker took the match over; nothing was written."""

    def __init__(self, match: MatchRow) -> None:
        super().__init__(f"match {match.id} is no longer held")


async def enqueue_match(db: DBConnection, user: User, req: MatchEnqueue) -> UUID:
    """Admins queue anything. A member queues submissions only, at least one their team's,
    at normal priority."""
    names: list[str] = []
    owned = False
    for bot in req.bots:
        if isinstance(bot, PathSource):
            if not user.is_admin:
                raise Forbidden("only admins may run bots from a path")
            names.append(bot.name)
        else:
            submission = await get_submission(db, bot.id)
            if submission.game != req.game:
                raise ApiError(f"submission {bot.id} is for {submission.game}, not {req.game}")
            owned = owned or submission.team_id == user.team_id
            names.append(submission.name)
    if not user.is_admin and not owned:
        raise Forbidden("one of the bots must be your team's submission")
    priority = req.priority if user.is_admin else 0
    teams = [TeamInfo(id=i, name=name) for i, name in enumerate(names)]
    return await db.match_repo.insert("user", req.game, teams, req.config, req.bots, priority)


async def get_match(db: DBConnection, user: User, match_id: UUID) -> Match:
    row = await _visible(db, user, match_id)
    sets = await db.match_repo.list_set_results(match_id)
    return Match(**row.model_dump(), sets=sets)


async def get_set_replay(db: DBConnection, user: User, match_id: UUID, index: int) -> SetReplay:
    await _visible(db, user, match_id)
    replay = await db.match_repo.get_set_replay(match_id, index)
    if replay is None:
        raise NotFound(f"no set {index} in match {match_id}")
    return replay


async def list_matches(db: DBConnection, user: User, mine: bool = False) -> list[MatchRow]:
    """Platform matches and matches with one of the caller's team's submissions, newest
    first; every match for an admin. `mine` narrows to the team's own, admin or not."""
    if mine:
        return await db.match_repo.list_recent(user.team_id, LIST_LIMIT, owned_only=True)
    return await db.match_repo.list_recent(None if user.is_admin else user.team_id, LIST_LIMIT)


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


async def finish_match(db: DBConnection, match: MatchRow, replay: MatchReplay) -> None:
    """Records the replay and marks the match done. Sets from an earlier attempt are
    replaced, so a rerun after a lost worker leaves one consistent replay."""
    async with db.conn.transaction():
        await db.match_repo.delete_sets(match.id)
        for s in replay.sets:
            await db.match_repo.add_set(match.id, s)
        done = await db.match_repo.complete(
            match.id,
            _claimed_at(match),
            replay.result.set_wins,
            replay.result.winner_team,
            replay.engine_version,
        )
        if not done:
            raise LostLease(match)


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
