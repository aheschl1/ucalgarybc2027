from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.users import StoredUser

COLUMNS = "id, email, display_name, password_hash, is_admin, team_id, created_at"


class UserRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(
        self, email: str, display_name: str, password_hash: str, is_admin: bool, team_id: int
    ) -> StoredUser:
        """Raises `psycopg.errors.UniqueViolation` when the email has an account."""
        cur = await self._conn.execute(
            f"insert into users (email, display_name, password_hash, is_admin, team_id) "
            f"values (%s, %s, %s, %s, %s) returning {COLUMNS}",
            (email, display_name, password_hash, is_admin, team_id),
        )
        row = await cur.fetchone()
        assert row is not None
        return StoredUser.model_validate(row)

    async def set_team(self, id: int, team_id: int) -> None:
        await self._conn.execute("update users set team_id = %s where id = %s", (team_id, id))

    async def get_by_email(self, email: str) -> StoredUser | None:
        cur = await self._conn.execute(f"select {COLUMNS} from users where email = %s", (email,))
        row = await cur.fetchone()
        return None if row is None else StoredUser.model_validate(row)

    async def list(self) -> list[StoredUser]:
        cur = await self._conn.execute(f"select {COLUMNS} from users order by id")
        return [StoredUser.model_validate(row) for row in await cur.fetchall()]
