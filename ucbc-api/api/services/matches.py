from datetime import datetime, timedelta
from uuid import UUID

from api.db import DBConnection
from api.errors import NotFound
from api.models.matches import Match, MatchEnqueue, MatchReplay, MatchRow, SetReplay, TeamInfo


class LostLease(Exception):
    """Another worker took the match over; nothing was written."""

    def __init__(self, match: MatchRow) -> None:
        super().__init__(f"match {match.id} is no longer held")


async def enqueue_match(db: DBConnection, req: MatchEnqueue) -> UUID:
    teams = [TeamInfo(id=i, name=bot.name) for i, bot in enumerate(req.bots)]
    return await db.match_repo.insert(req.game, teams, req.config, req.bots, req.priority)


async def get_match(db: DBConnection, match_id: UUID) -> Match:
    row = await db.match_repo.get(match_id)
    if row is None:
        raise NotFound(f"no match {match_id}")
    sets = await db.match_repo.list_set_results(match_id)
    return Match(**row.model_dump(), sets=sets)


async def get_set_replay(db: DBConnection, match_id: UUID, index: int) -> SetReplay:
    replay = await db.match_repo.get_set_replay(match_id, index)
    if replay is None:
        raise NotFound(f"no set {index} in match {match_id}")
    return replay


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
