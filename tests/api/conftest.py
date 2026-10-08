"""API tests run against a throwaway Postgres from testcontainers. The schema comes from
`alembic upgrade head`; tables are emptied before each test."""

import os
import subprocess
import sys
from collections.abc import AsyncIterator, Iterator
from contextlib import asynccontextmanager
from pathlib import Path

import pytest
from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient
from testcontainers.community.minio import MinioContainer
from testcontainers.community.postgres import PostgresContainer

from api.api import create_app
from api.blobs import BlobStore
from api.db import DBConnection
from api.models.users import User
from api.routes.auth import limiter
from api.services.users import create_user

ROOT = Path(__file__).resolve().parents[2]
ADMIN = ("root@example.com", "root-pw")
MEMBER = ("alice@example.com", "alice-pw")
TEAMMATE = ("carol@example.com", "carol-pw")


def alembic(url: str, *args: str) -> None:
    subprocess.run(
        [sys.executable, "-m", "alembic", *args],
        cwd=ROOT,
        env={**os.environ, "UCBC_DATABASE_URL": url},
        check=True,
        capture_output=True,
    )


@pytest.fixture(scope="session")
def database_url() -> Iterator[str]:
    with PostgresContainer("postgres:17", driver=None) as pg:
        url = pg.get_connection_url()
        alembic(url, "upgrade", "head")
        yield url


@pytest.fixture(scope="session")
def blob_url() -> Iterator[str]:
    with MinioContainer("pgsty/minio") as minio:
        cfg = minio.get_config()
        yield f"http://{cfg['access_key']}:{cfg['secret_key']}@{cfg['endpoint']}/test"


@pytest.fixture
async def app(database_url: str, blob_url: str) -> AsyncIterator[FastAPI]:
    app = create_app(database_url, blob_url)
    # The login limiter's counters are process-wide, so each test starts from zero.
    limiter.reset()
    async with app.router.lifespan_context(app):
        async with app.state.pool.connection() as conn:
            await conn.execute(
                "truncate users, teams, sessions, matches, sets, submissions, maps "
                "restart identity cascade"
            )
        yield app


@pytest.fixture
async def db(app: FastAPI) -> AsyncIterator[DBConnection]:
    """A setup connection; autocommit so requests see its writes at once."""
    async with app.state.pool.connection() as conn:
        await conn.set_autocommit(True)
        yield DBConnection(conn)


@pytest.fixture
def blobs(app: FastAPI) -> BlobStore:
    """The app's blob store, for calling services that write replays directly."""
    store: BlobStore = app.state.blobs
    return store


def anonymous(app: FastAPI) -> AsyncClient:
    # The session cookie is Secure, so the client only sends it back over https.
    return AsyncClient(transport=ASGITransport(app=app), base_url="https://test")


@asynccontextmanager
async def log_in(app: FastAPI, creds: tuple[str, str]) -> AsyncIterator[AsyncClient]:
    """A client that logged in through the API and carries the session cookie."""
    async with anonymous(app) as c:
        r = await c.post("/auth/login", json={"email": creds[0], "password": creds[1]})
        assert r.status_code == 200, r.text
        yield c


@pytest.fixture
async def client(app: FastAPI) -> AsyncIterator[AsyncClient]:
    async with anonymous(app) as c:
        yield c


@pytest.fixture
async def admin(db: DBConnection) -> User:
    return await create_user(db, ADMIN[0], "Root", ADMIN[1], is_admin=True)


@pytest.fixture
async def member(db: DBConnection) -> User:
    return await create_user(db, MEMBER[0], "Alice", MEMBER[1], is_admin=False)


@pytest.fixture
async def admin_client(app: FastAPI, admin: User) -> AsyncIterator[AsyncClient]:
    async with log_in(app, ADMIN) as c:
        yield c


@pytest.fixture
async def member_client(app: FastAPI, member: User) -> AsyncIterator[AsyncClient]:
    async with log_in(app, MEMBER) as c:
        yield c


@pytest.fixture
async def teammate_client(
    app: FastAPI, db: DBConnection, member_client: AsyncClient
) -> AsyncIterator[AsyncClient]:
    """Carol, who joined the member's team with its code."""
    await create_user(db, TEAMMATE[0], "Carol", TEAMMATE[1], is_admin=False)
    code = (await member_client.get("/teams/me")).json()["join_code"]
    async with log_in(app, TEAMMATE) as c:
        assert (await c.post("/teams/join", json={"code": code})).status_code == 200
        yield c
