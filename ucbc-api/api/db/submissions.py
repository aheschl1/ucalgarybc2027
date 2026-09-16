from uuid import UUID

from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.submissions import Submission

COLUMNS = (
    "s.id, s.user_id, s.name, s.game, s.size, s.sha256, s.created_at, u.display_name "
    "from submissions s join users u on u.id = s.user_id"
)


class SubmissionRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(
        self, id: UUID, user_id: int, name: str, game: str, size: int, sha256: str
    ) -> None:
        await self._conn.execute(
            "insert into submissions (id, user_id, name, game, size, sha256) "
            "values (%s, %s, %s, %s, %s, %s)",
            (id, user_id, name, game, size, sha256),
        )

    async def get(self, id: UUID) -> Submission | None:
        cur = await self._conn.execute(f"select {COLUMNS} where s.id = %s", (id,))
        row = await cur.fetchone()
        return None if row is None else Submission.model_validate(row)

    async def list_recent(self, user_id: int | None) -> list[Submission]:
        """Newest first; every user's when `user_id` is None."""
        cur = await self._conn.execute(
            f"select {COLUMNS} where %s::bigint is null or s.user_id = %s "
            "order by s.created_at desc",
            (user_id, user_id),
        )
        return [Submission.model_validate(row) for row in await cur.fetchall()]
