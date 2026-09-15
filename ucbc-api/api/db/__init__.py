"""One `DBConnection` per request or CLI command, holding a repo per table."""

from collections.abc import AsyncIterator
from typing import Annotated

from fastapi import Depends, Request
from psycopg import AsyncConnection
from psycopg.rows import DictRow, dict_row
from psycopg_pool import AsyncConnectionPool

from api.db.users import UserRepo


class DBConnection:
    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self.conn = conn
        self.user_repo = UserRepo(conn)


def create_pool(database_url: str) -> AsyncConnectionPool[AsyncConnection[DictRow]]:
    return AsyncConnectionPool(
        database_url,
        open=False,
        connection_class=AsyncConnection[DictRow],
        kwargs={"row_factory": dict_row},
    )


async def get_db(request: Request) -> AsyncIterator[DBConnection]:
    """One connection per request; committed on success, rolled back on error. The commit
    must land before the response is sent, hence `scope="function"` below."""
    pool: AsyncConnectionPool[AsyncConnection[DictRow]] = request.app.state.pool
    async with pool.connection() as conn:
        yield DBConnection(conn)


DB = Annotated[DBConnection, Depends(get_db, scope="function")]
