from typing import Any
from uuid import UUID

from psycopg.errors import UniqueViolation

from api.db import DBConnection
from api.errors import Conflict, NotFound
from api.models import Match, MatchCreate, MatchResult, SetReplay


async def create_match(db: DBConnection, match: MatchCreate) -> UUID:
    return await db.match_repo.insert(match)


async def _running(db: DBConnection, match_id: UUID) -> dict[str, Any]:
    row = await db.match_repo.get(match_id)
    if row is None:
        raise NotFound(f"no match {match_id}")
    if row["status"] == "done":
        raise Conflict(f"match {match_id} is done")
    return row


async def add_set(db: DBConnection, match_id: UUID, replay: SetReplay) -> None:
    await _running(db, match_id)
    try:
        await db.match_repo.add_set(match_id, replay)
    except UniqueViolation as e:
        raise Conflict(f"match {match_id} already has set {replay.index}") from e


async def complete_match(db: DBConnection, match_id: UUID, result: MatchResult) -> None:
    await _running(db, match_id)
    stored = await db.match_repo.list_set_results(match_id)
    if len(stored) != len(result.sets):
        raise Conflict(
            f"match {match_id} has {len(stored)} sets stored, result names {len(result.sets)}"
        )
    await db.match_repo.complete(match_id, result.set_wins, result.winner_team)


async def get_match(db: DBConnection, match_id: UUID) -> Match:
    row = await db.match_repo.get(match_id)
    if row is None:
        raise NotFound(f"no match {match_id}")
    sets = await db.match_repo.list_set_results(match_id)
    return Match.model_validate({**row, "sets": sets})


async def get_set_replay(db: DBConnection, match_id: UUID, index: int) -> dict[str, Any]:
    replay = await db.match_repo.get_set_replay(match_id, index)
    if replay is None:
        raise NotFound(f"no set {index} in match {match_id}")
    return replay
