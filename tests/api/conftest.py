"""API tests run against a real Postgres named by UCBC_TEST_DATABASE_URL and are skipped
without it. The schema comes from `alembic upgrade head`; tables are emptied before each test."""

import os
import subprocess
import sys
from collections.abc import AsyncIterator
from pathlib import Path

import pytest
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient

from api.api import create_app
from api.db import DBConnection
from api.models import User
from api.services.users import create_user

ROOT = Path(__file__).resolve().parents[2]
ADMIN = ("root", "root-pw")
MEMBER = ("alice", "alice-pw")


@pytest.fixture(scope="session")
def database_url() -> str:
    url = os.environ.get("UCBC_TEST_DATABASE_URL")
    if not url:
        pytest.skip("UCBC_TEST_DATABASE_URL is not set")
    subprocess.run(
        [sys.executable, "-m", "alembic", "upgrade", "head"],
        cwd=ROOT,
        env={**os.environ, "UCBC_DATABASE_URL": url},
        check=True,
        capture_output=True,
    )
    return url


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
