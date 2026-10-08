from fastapi import FastAPI
from httpx import AsyncClient

from api.db import DBConnection
from tests.api.conftest import MEMBER, log_in

SIGNUP = {"email": "bob@example.com", "display_name": "Bob", "password": "bob-password"}


async def test_health(client: AsyncClient) -> None:
    r = await client.get("/health")
    assert r.status_code == 200
    assert r.json() == {"status": "ok"}


async def test_me(client: AsyncClient, member_client: AsyncClient) -> None:
    assert (await client.get("/users/me")).status_code == 401
    r = await member_client.get("/users/me")
    assert r.status_code == 200
    body = r.json()
    assert body["email"] == MEMBER[0]
    assert body["display_name"] == "Alice"
    assert body["is_admin"] is False
    assert isinstance(body["team_id"], int)
    assert "password_hash" not in body


async def team_name(db: DBConnection, team_id: int) -> str:
    cur = await db.conn.execute("select name from teams where id = %s", (team_id,))
    row = await cur.fetchone()
    assert row is not None
    name: str = row["name"]
    return name


async def test_signup_is_public(app: FastAPI, client: AsyncClient) -> None:
    r = await client.post("/users", json=SIGNUP)
    assert r.status_code == 201, r.text
    body = r.json()
    assert body["email"] == SIGNUP["email"]
    assert body["display_name"] == "Bob"
    assert body["is_admin"] is False
    assert "password_hash" not in body

    # Signing up does not log you in; the app logs in straight after.
    assert (await client.get("/users/me")).status_code == 401
    async with log_in(app, (SIGNUP["email"], SIGNUP["password"])) as bob:
        assert (await bob.get("/users/me")).json()["display_name"] == "Bob"


async def test_signup_makes_own_team(client: AsyncClient, db: DBConnection) -> None:
    # Team names are unique ignoring case, so a clashing display name gets a number.
    teams = []
    for i, name in enumerate(["Bob", "Bob", "BOB"]):
        signup = {**SIGNUP, "email": f"bob{i}@example.com", "display_name": name}
        r = await client.post("/users", json=signup)
        assert r.status_code == 201, r.text
        teams.append(r.json()["team_id"])
    assert len(set(teams)) == 3
    assert [await team_name(db, t) for t in teams] == ["Bob", "Bob 2", "BOB 3"]


async def test_failed_signup_leaves_no_team(client: AsyncClient, db: DBConnection) -> None:
    assert (await client.post("/users", json=SIGNUP)).status_code == 201
    r = await client.post("/users", json={**SIGNUP, "display_name": "Other"})
    assert r.status_code == 409
    cur = await db.conn.execute("select name from teams")
    assert [row["name"] for row in await cur.fetchall()] == ["Bob"]


async def test_signup_cannot_self_promote(app: FastAPI, client: AsyncClient) -> None:
    r = await client.post("/users", json={**SIGNUP, "is_admin": True})
    assert r.status_code == 201
    assert r.json()["is_admin"] is False


async def test_one_account_per_address(client: AsyncClient) -> None:
    assert (await client.post("/users", json=SIGNUP)).status_code == 201
    # The address is normalised, so a different spelling is the same account.
    r = await client.post("/users", json={**SIGNUP, "email": "BOB@EXAMPLE.com"})
    assert r.status_code == 409
    assert "already has an account" in r.json()["detail"]


async def test_signup_validation(client: AsyncClient) -> None:
    for bad in [
        {"email": "not-an-address"},
        {"password": "short"},
        {"display_name": ""},
        {"display_name": "x" * 65},
    ]:
        assert (await client.post("/users", json={**SIGNUP, **bad})).status_code == 422
