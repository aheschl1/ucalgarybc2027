"""Participant teams: every user is on one, and moves by creating or joining another."""

from collections.abc import AsyncIterator
from typing import Any

import pytest
from fastapi import FastAPI
from httpx import AsyncClient

from api.db import DBConnection
from api.services.users import create_user
from tests.api.conftest import log_in
from tests.api.test_submissions import upload, zip_of

BOB = ("bob@example.com", "bob-pw")


@pytest.fixture
async def bob_client(app: FastAPI, db: DBConnection) -> AsyncIterator[AsyncClient]:
    """A member on a team of his own."""
    await create_user(db, BOB[0], "Bob", BOB[1], is_admin=False)
    async with log_in(app, BOB) as c:
        yield c


async def my_team(client: AsyncClient) -> Any:
    r = await client.get("/teams/me")
    assert r.status_code == 200, r.text
    return r.json()


async def test_my_team(member_client: AsyncClient) -> None:
    team = await my_team(member_client)
    assert team["name"] == "Alice"
    assert team["members"] == ["Alice"]
    assert team["join_code"]
    assert "alice@example.com" not in str(team)


async def test_join_by_code(member_client: AsyncClient, bob_client: AsyncClient) -> None:
    alice = await my_team(member_client)
    r = await bob_client.post("/teams/join", json={"code": alice["join_code"]})
    assert r.status_code == 200, r.text
    assert r.json()["id"] == alice["id"]
    assert r.json()["members"] == ["Alice", "Bob"]
    assert await my_team(member_client) == await my_team(bob_client)
    assert (await bob_client.get("/users/me")).json()["team_id"] == alice["id"]

    # Joining again changes nothing.
    r = await bob_client.post("/teams/join", json={"code": alice["join_code"]})
    assert r.json()["members"] == ["Alice", "Bob"]

    # Bob's old team is left empty but keeps its name.
    assert (await bob_client.post("/teams", json={"name": "bob"})).status_code == 409


async def test_bad_code(member_client: AsyncClient) -> None:
    for code in ["nope", ""]:
        r = await member_client.post("/teams/join", json={"code": code})
        assert r.status_code == 404
    assert (await my_team(member_client))["name"] == "Alice"


async def test_create_moves_caller(
    member_client: AsyncClient, bob_client: AsyncClient, db: DBConnection
) -> None:
    old = await my_team(member_client)
    r = await upload(member_client, zip_of({"main.py": b""}))
    assert r.status_code == 201
    submission = r.json()["id"]

    r = await member_client.post("/teams", json={"name": "  Rustaceans "})
    assert r.status_code == 201, r.text
    team = r.json()
    assert team["name"] == "Rustaceans"
    assert team["members"] == ["Alice"]
    assert team["join_code"] != old["join_code"]
    assert (await member_client.get("/users/me")).json()["team_id"] == team["id"]

    # The old team keeps the submission.
    cur = await db.conn.execute("select team_id from submissions where id = %s", (submission,))
    assert await cur.fetchone() == {"team_id": old["id"]}

    # Names are unique ignoring case, the emptied team's included.
    for name in ["rustaceans", "ALICE"]:
        r = await bob_client.post("/teams", json={"name": name})
        assert r.status_code == 409, name
        assert "already called" in r.json()["detail"]
    for name in ["", "   ", "x" * 65]:
        assert (await bob_client.post("/teams", json={"name": name})).status_code == 422
    assert (await my_team(bob_client))["name"] == "Bob"


async def test_replace_code(member_client: AsyncClient, bob_client: AsyncClient) -> None:
    alice = await my_team(member_client)
    bob = await my_team(bob_client)
    r = await member_client.post("/teams/me/code")
    assert r.status_code == 200
    code = r.json()["join_code"]
    assert code != alice["join_code"]
    # Only the caller's own team changes.
    assert (await my_team(bob_client))["join_code"] == bob["join_code"]

    r = await bob_client.post("/teams/join", json={"code": alice["join_code"]})
    assert r.status_code == 404
    r = await bob_client.post("/teams/join", json={"code": code})
    assert r.json()["id"] == alice["id"]


async def test_needs_login(client: AsyncClient) -> None:
    assert (await client.get("/teams/me")).status_code == 401
    assert (await client.post("/teams", json={"name": "x"})).status_code == 401
    assert (await client.post("/teams/join", json={"code": "x"})).status_code == 401
    assert (await client.post("/teams/me/code")).status_code == 401
