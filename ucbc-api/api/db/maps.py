from uuid import UUID

from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.maps import Map
from api.errors import ApiError

COLUMNS = "id, game, name, size, sha256, uploaded_by, created_at, archived_at"


class MapRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(
        self, id: UUID, game: str, name: str, size: int, sha256: str, uploaded_by: int | None
    ) -> None:
        """Raises `psycopg.errors.UniqueViolation` when the game has a map of that name,
        ignoring case."""
        await self._conn.execute(
            "insert into maps (id, game, name, size, sha256, uploaded_by) "
            "values (%s, %s, %s, %s, %s, %s)",
            (id, game, name, size, sha256, uploaded_by),
        )

    async def get(self, id: UUID) -> Map | None:
        cur = await self._conn.execute(f"select {COLUMNS} from maps where id = %s", (id,))
        row = await cur.fetchone()
        return None if row is None else Map.model_validate(row)

    async def list_maps(self, game: str | None) -> list[Map]:
        """By game, then name; every game's when `game` is None."""
        cur = await self._conn.execute(
            f"select {COLUMNS} from maps where %s::text is null or game = %s "
            "order by game, lower(name)",
            (game, game),
        )
        return [Map.model_validate(row) for row in await cur.fetchall()]

    async def set_archived(self, id: UUID, archived: bool) -> bool:
        """Archiving an archived map keeps its first `archived_at`."""
        cur = await self._conn.execute(
            "update maps set archived_at = "
            "case when %s then coalesce(archived_at, now()) else null end where id = %s",
            (archived, id),
        )
        return cur.rowcount == 1

    async def get_map_uuids(self, game: str, n: int) -> list[UUID]:
        result = await self._conn.execute(
            """
            select id from maps
            order by RAND()
            where game = %s
            limit %s;
            """,
            (game, n,)
        )
        maps = result.fetchall()
        if len(maps) == 0:
            raise ApiError()
        elif len(maps) < n:
            maps = maps[:-1] + maps[-1] * (n - len(maps) + 1)
        return [row["id"] for row in maps]
