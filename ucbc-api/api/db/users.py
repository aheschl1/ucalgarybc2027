from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.users import StoredUser

COLUMNS = "id, username, password_hash, is_admin, created_at"


class UserRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(self, username: str, password_hash: str, is_admin: bool) -> StoredUser:
        """Raises `psycopg.errors.UniqueViolation` when the username exists."""
        cur = await self._conn.execute(
            f"insert into users (username, password_hash, is_admin) values (%s, %s, %s) "
            f"returning {COLUMNS}",
            (username, password_hash, is_admin),
        )
        row = await cur.fetchone()
        assert row is not None
        return StoredUser.model_validate(row)

    async def get_by_username(self, username: str) -> StoredUser | None:
        cur = await self._conn.execute(
            f"select {COLUMNS} from users where username = %s", (username,)
        )
        row = await cur.fetchone()
        return None if row is None else StoredUser.model_validate(row)

    async def list(self) -> list[StoredUser]:
        cur = await self._conn.execute(f"select {COLUMNS} from users order by id")
        return [StoredUser.model_validate(row) for row in await cur.fetchall()]
