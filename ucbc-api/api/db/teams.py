from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.teams import Team

COLUMNS = "id, name, created_at"


class TeamRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(self, name: str) -> Team:
        """Raises `psycopg.errors.UniqueViolation` when a team has the name, in any case."""
        cur = await self._conn.execute(
            f"insert into teams (name) values (%s) returning {COLUMNS}", (name,)
        )
        row = await cur.fetchone()
        assert row is not None
        return Team.model_validate(row)
