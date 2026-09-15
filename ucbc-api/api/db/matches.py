from typing import Any
from uuid import UUID

from psycopg import AsyncConnection
from psycopg.rows import DictRow
from psycopg.types.json import Jsonb

from api.models import MatchCreate, SetReplay, SetResult

SET_RESULT_COLUMNS = "index, first_team, winner_team, reason, detail, ticks"


class MatchRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(self, match: MatchCreate) -> UUID:
        cur = await self._conn.execute(
            "insert into matches (game, engine_version, teams, config, status) "
            "values (%s, %s, %s, %s, 'running') returning id",
            (
                match.game,
                match.engine_version,
                Jsonb([t.model_dump() for t in match.teams]),
                Jsonb(match.config),
            ),
        )
        row = await cur.fetchone()
        assert row is not None
        id: UUID = row["id"]
        return id

    async def get(self, match_id: UUID) -> dict[str, Any] | None:
        """The match row without its sets."""
        cur = await self._conn.execute(
            "select id, game, engine_version, teams, config, status, set_wins, winner_team, "
            "created_at, completed_at from matches where id = %s",
            (match_id,),
        )
        return await cur.fetchone()

    async def add_set(self, match_id: UUID, replay: SetReplay) -> None:
        """Raises `psycopg.errors.UniqueViolation` when the set index exists."""
        r = replay.result
        await self._conn.execute(
            "insert into sets (match_id, index, first_team, winner_team, reason, detail, ticks, "
            "replay) values (%s, %s, %s, %s, %s, %s, %s, %s)",
            (
                match_id,
                r.index,
                r.first_team,
                r.winner_team,
                r.reason,
                r.detail,
                r.ticks,
                Jsonb(replay.model_dump(exclude_unset=True)),
            ),
        )

    async def list_set_results(self, match_id: UUID) -> list[SetResult]:
        cur = await self._conn.execute(
            f"select {SET_RESULT_COLUMNS} from sets where match_id = %s order by index",
            (match_id,),
        )
        return [SetResult.model_validate(row) for row in await cur.fetchall()]

    async def get_set_replay(self, match_id: UUID, index: int) -> dict[str, Any] | None:
        cur = await self._conn.execute(
            "select replay from sets where match_id = %s and index = %s", (match_id, index)
        )
        row = await cur.fetchone()
        if row is None:
            return None
        replay: dict[str, Any] = row["replay"]
        return replay

    async def complete(self, match_id: UUID, set_wins: list[int], winner_team: int | None) -> None:
        await self._conn.execute(
            "update matches set status = 'done', set_wins = %s, winner_team = %s, "
            "completed_at = now() where id = %s",
            (Jsonb(set_wins), winner_team, match_id),
        )
