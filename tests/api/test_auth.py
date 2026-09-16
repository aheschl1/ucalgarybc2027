"""Logging in and out: the session cookie and what invalidates it."""

from datetime import UTC, datetime, timedelta

from httpx import AsyncClient

from api.db import DBConnection
from api.models.users import User
from api.services.sessions import COOKIE, hash_token
from tests.api.conftest import MEMBER


def credentials(username: str, password: str) -> dict[str, str]:
    return {"username": username, "password": password}


async def test_login_sets_a_same_origin_cookie(client: AsyncClient, member: User) -> None:
    r = await client.post("/auth/login", json=credentials(*MEMBER))
    assert r.status_code == 200
    assert r.json()["username"] == MEMBER[0]
    assert "password_hash" not in r.json()
    cookie = r.headers["set-cookie"].lower()
    assert cookie.startswith(f"{COOKIE}=")
    assert "httponly" in cookie
    assert "samesite=strict" in cookie
    assert "path=/" in cookie
    assert "max-age=2592000" in cookie
    assert "domain" not in cookie
    assert "secure" in cookie

    # The client keeps the cookie, and nothing else is needed on later requests.
    r = await client.get("/users/me")
    assert r.status_code == 200
    assert r.json()["username"] == MEMBER[0]


async def test_wrong_credentials_get_no_cookie(client: AsyncClient, member: User) -> None:
    for body in [credentials(MEMBER[0], "wrong"), credentials("nobody", MEMBER[1])]:
        r = await client.post("/auth/login", json=body)
        assert r.status_code == 401
        assert "set-cookie" not in r.headers
    assert (await client.get("/users/me")).status_code == 401
    assert "www-authenticate" not in (await client.get("/users/me")).headers


async def test_logout_ends_the_session(client: AsyncClient, member: User) -> None:
    r = await client.post("/auth/login", json=credentials(*MEMBER))
    token = r.cookies[COOKIE]
    assert (await client.get("/users/me")).status_code == 200

    r = await client.post("/auth/logout")
    assert r.status_code == 204
    assert "max-age=0" in r.headers["set-cookie"].lower()
    assert COOKIE not in client.cookies
    assert (await client.get("/users/me")).status_code == 401
    # The row is gone, so replaying the old token fails too.
    r = await client.get("/users/me", headers={"Cookie": f"{COOKIE}={token}"})
    assert r.status_code == 401

    # Logging out without a session is fine.
    assert (await client.post("/auth/logout")).status_code == 204


async def test_expired_and_unknown_sessions(
    client: AsyncClient, db: DBConnection, member: User
) -> None:
    await db.session_repo.insert(
        hash_token("old"), member.id, datetime.now(UTC) - timedelta(seconds=1)
    )
    for token in ["old", "unknown"]:
        r = await client.get("/users/me", headers={"Cookie": f"{COOKIE}={token}"})
        assert r.status_code == 401

    # Logging in sweeps the user's expired rows.
    await client.post("/auth/login", json=credentials(*MEMBER))
    assert await db.session_repo.get_user(hash_token("old")) is None
    cur = await db.conn.execute("select count(*) from sessions where user_id = %s", (member.id,))
    row = await cur.fetchone()
    assert row is not None and row["count"] == 1


async def test_deleted_user_loses_the_session(
    member_client: AsyncClient, db: DBConnection, member: User
) -> None:
    assert (await member_client.get("/users/me")).status_code == 200
    await db.conn.execute("delete from users where id = %s", (member.id,))
    assert (await member_client.get("/users/me")).status_code == 401
