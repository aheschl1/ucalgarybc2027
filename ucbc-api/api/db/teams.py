from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.teams import Team

COLUMNS = "id, name, join_code, created_at"


class TeamRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(self, name: str, join_code: str) -> Team:
        """Raises `psycopg.errors.UniqueViolation` when a team has the name, in any case."""
        cur = await self._conn.execute(
            f"insert into teams (name, join_code) values (%s, %s) returning {COLUMNS}",
            (name, join_code),
        )
        row = await cur.fetchone()
        assert row is not None
        return Team.model_validate(row)

    async def get(self, id: int) -> Team | None:
        cur = await self._conn.execute(f"select {COLUMNS} from teams where id = %s", (id,))
        row = await cur.fetchone()
        return None if row is None else Team.model_validate(row)

    async def get_by_code(self, join_code: str) -> Team | None:
        cur = await self._conn.execute(
            f"select {COLUMNS} from teams where join_code = %s", (join_code,)
        )
        row = await cur.fetchone()
        return None if row is None else Team.model_validate(row)

    async def set_code(self, id: int, join_code: str) -> None:
        await self._conn.execute("update teams set join_code = %s where id = %s", (join_code, id))

    async def lock_elos(self, ids: list[int]) -> dict[int, float]:
        """The teams' ratings, locked until the transaction ends. Rows lock in id order, so
        two transactions locking the same teams cannot deadlock; `no key` lets foreign key
        checks, such as an upload to the team, go ahead meanwhile."""
        cur = await self._conn.execute(
            "select id, elo from teams where id = any(%s) order by id for no key update", (ids,)
        )
        return {row["id"]: row["elo"] for row in await cur.fetchall()}

    async def record_elo(self, id: int, elo: float) -> None:
        """The rating after one more rated match."""
        await self._conn.execute(
            "update teams set elo = %s, elo_matches = elo_matches + 1 where id = %s", (elo, id)
        )

    async def members(self, id: int) -> list[str]:
        """Display names, oldest account first."""
        cur = await self._conn.execute(
            "select display_name from users where team_id = %s order by id", (id,)
        )
        return [row["display_name"] for row in await cur.fetchall()]
