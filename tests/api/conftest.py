"""API tests run against a throwaway Postgres from testcontainers. The schema comes from
`alembic upgrade head`; tables are emptied before each test."""

import os
import subprocess
import sys
from collections.abc import AsyncIterator, Iterator
from pathlib import Path

import pytest
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from testcontainers.community.postgres import PostgresContainer

from api.api import create_app
from api.db import DBConnection
from api.models.users import User
from api.services.users import create_user

ROOT = Path(__file__).resolve().parents[2]
ADMIN = ("root", "root-pw")
MEMBER = ("alice", "alice-pw")


@pytest.fixture(scope="session")
def database_url() -> Iterator[str]:
    with PostgresContainer("postgres:17", driver=None) as pg:
        url = pg.get_connection_url()
        subprocess.run(
            [sys.executable, "-m", "alembic", "upgrade", "head"],
            cwd=ROOT,
            env={**os.environ, "UCBC_DATABASE_URL": url},
            check=True,
            capture_output=True,
        )
        yield url


@pytest.fixture
async def app(database_url: str) -> AsyncIterator[FastAPI]:
    app = create_app(database_url)
    async with app.router.lifespan_context(app):
        async with app.state.pool.connection() as conn:
            await conn.execute("truncate users, matches, sets restart identity cascade")
        yield app


@pytest.fixture
async def db(app: FastAPI) -> AsyncIterator[DBConnection]:
    """A setup connection; autocommit so requests see its writes at once."""
    async with app.state.pool.connection() as conn:
        await conn.set_autocommit(True)
        yield DBConnection(conn)


@pytest.fixture
async def client(app: FastAPI) -> AsyncIterator[AsyncClient]:
    async with AsyncClient(transport=ASGITransport(app=app), base_url="http://test") as c:
        yield c


@pytest.fixture
async def admin(db: DBConnection) -> User:
    return await create_user(db, *ADMIN, is_admin=True)


@pytest.fixture
async def member(db: DBConnection) -> User:
    return await create_user(db, *MEMBER, is_admin=False)
