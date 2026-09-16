from datetime import datetime

from psycopg import AsyncConnection
from psycopg.rows import DictRow

from api.models.users import StoredUser


class SessionRepo:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(self, token_hash: str, user_id: int, expires_at: datetime) -> None:
        await self._conn.execute(
            "insert into sessions (token_hash, user_id, expires_at) values (%s, %s, %s)",
            (token_hash, user_id, expires_at),
        )

    async def get_user(self, token_hash: str) -> StoredUser | None:
        """The user behind a live session, or None when it is unknown or expired."""
        cur = await self._conn.execute(
            "select u.id, u.username, u.password_hash, u.is_admin, u.created_at "
            "from sessions s join users u on u.id = s.user_id "
            "where s.token_hash = %s and s.expires_at > now()",
            (token_hash,),
        )
        row = await cur.fetchone()
        return None if row is None else StoredUser.model_validate(row)

    async def delete(self, token_hash: str) -> None:
        await self._conn.execute("delete from sessions where token_hash = %s", (token_hash,))

    async def delete_expired(self, user_id: int) -> None:
        await self._conn.execute(
            "delete from sessions where user_id = %s and expires_at <= now()", (user_id,)
        )
