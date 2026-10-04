"""The match queue: enqueue and read through the API, claim and lease through the services."""

import gzip
import json
from collections.abc import AsyncIterator, Callable
from datetime import timedelta
from pathlib import Path
from typing import Any
from uuid import UUID

import pytest
from fastapi import FastAPI
from httpx import AsyncClient

from api.db import DBConnection
from api.models.matches import MatchReplay, SetReplay
from api.services import matches
from api.services.users import create_user
from tests.api.conftest import log_in
from tests.api.test_submissions import upload, zip_dir

LEASE = timedelta(seconds=30)
MISSING = "00000000-0000-0000-0000-000000000000"


def set_replay(index: int, winner: int | None) -> dict[str, Any]:
    result = {
        "index": index,
        "first_team": index % 2,
        "winner_team": winner,
        "reason": "draw" if winner is None else "win",
        "ticks": 1,
    }
    if winner is None:
        result["detail"] = "board full"
    return {
        "index": index,
        "first_team": index % 2,
        "initial_state": {"cells": ["empty"] * 9},
        "ticks": [{"number": 0, "steps": [], "state_after": {"cells": ["x"] + ["empty"] * 8}}],
        "result": result,
    }


def replay(match_id: UUID) -> MatchReplay:
    sets = [set_replay(i, w) for i, w in enumerate([None, 0, 0])]
    return MatchReplay(
        match_id=str(match_id),
        engine_version="0.1.0",
        config={},
        teams=[{"id": 0, "name": "random"}, {"id": 1, "name": "first_empty"}],
        sets=sets,
        result={"sets": [s["result"] for s in sets], "set_wins": [2, 0], "winner_team": 0},
    )


@pytest.fixture
async def other(app: FastAPI) -> AsyncIterator[DBConnection]:
    """A second worker's connection."""
    async with app.state.pool.connection() as conn:
        await conn.set_autocommit(True)
        yield DBConnection(conn)


@pytest.fixture
async def body(admin_client: AsyncClient, bot: Callable[[str], Path]) -> dict[str, Any]:
    """A match between two of the admin's submissions."""
    ids = []
    for name in ["random", "first_empty"]:
        r = await upload(admin_client, zip_dir(bot(name)), form={"name": name})
        ids.append(r.json()["id"])
    return {"game": "tictactoe", "bots": ids, "config": {"seed": 7}}


async def enqueue(client: AsyncClient, body: dict[str, Any], **override: Any) -> UUID:
    r = await client.post("/matches/queue", json={**body, **override})
    assert r.status_code == 201, r.text
    return UUID(r.json()["id"])


async def get(client: AsyncClient, match_id: UUID) -> dict[str, Any]:
    r = await client.get(f"/matches/{match_id}")
    assert r.status_code == 200, r.text
    body: dict[str, Any] = r.json()
    return body


async def test_enqueue_and_get(admin_client: AsyncClient, body: dict[str, Any]) -> None:
    match_id = await enqueue(admin_client, body)

    match = await get(admin_client, match_id)
    assert match["status"] == "queued"
    assert match["engine_version"] is None
    assert match["bots"] == body["bots"]
    assert [t["name"] for t in match["teams"]] == ["random", "first_empty"]
    assert match["config"] == {"sets": 3, "seed": 7, "step_ms": 3, "memory_bytes": 2**30}
    assert match["attempts"] == 0
    assert match["claimed_by"] is None
    assert match["sets"] == []


async def test_permissions_and_validation(
    client: AsyncClient,
    admin_client: AsyncClient,
    member_client: AsyncClient,
    body: dict[str, Any],
) -> None:
    assert (await member_client.post("/matches/queue", json=body)).status_code == 403
    one_bot = {**body, "bots": body["bots"][:1]}
    assert (await admin_client.post("/matches/queue", json=one_bot)).status_code == 422
    # Bots are submission ids, even for an admin: no directories, no source objects.
    path = {"kind": "path", "path": "bots/tictactoe/random"}
    wrapped = {"kind": "submission", "id": body["bots"][0]}
    for bot in [path, wrapped]:
        bad = {**body, "bots": [bot, body["bots"][1]]}
        assert (await admin_client.post("/matches/queue", json=bad)).status_code == 422

    match_id = await enqueue(admin_client, body)
    assert (await client.get(f"/matches/{match_id}")).status_code == 401
    assert (await member_client.get(f"/matches/{match_id}")).status_code == 404
    assert (await admin_client.get(f"/matches/{MISSING}")).status_code == 404
    assert (await admin_client.get(f"/matches/{match_id}/sets/0")).status_code == 404


async def test_claim_is_exclusive(
    admin_client: AsyncClient, db: DBConnection, other: DBConnection, body: dict[str, Any]
) -> None:
    match_id = await enqueue(admin_client, body)

    held = await matches.claim_match(db, "w1", LEASE)
    assert held is not None and held.id == match_id
    assert held.status == "running"
    assert held.attempts == 1
    assert held.claimed_by == "w1"
    assert await matches.claim_match(other, "w2", LEASE) is None
    assert (await get(admin_client, match_id))["status"] == "running"

    await matches.finish_match(db, held, replay(match_id))
    assert await matches.claim_match(other, "w2", LEASE) is None

    match = await get(admin_client, match_id)
    assert match["status"] == "done"
    assert match["engine_version"] == "0.1.0"
    assert match["set_wins"] == [2, 0]
    assert match["winner_team"] == 0
    assert match["completed_at"] is not None
    expected = [{"detail": "", **set_replay(i, w)["result"]} for i, w in enumerate([None, 0, 0])]
    assert match["sets"] == expected

    # Set replays are stored gzipped and come back as the JSON that went in.
    stored = await db.match_repo.get_set_replay(match_id, 0)
    assert stored is not None and json.loads(gzip.decompress(stored)) == set_replay(0, None)
    r = await admin_client.get(f"/matches/{match_id}/sets/0")
    assert r.status_code == 200
    assert r.headers["content-encoding"] == "gzip"
    assert r.json() == set_replay(0, None)
    assert (await admin_client.get(f"/matches/{match_id}/sets/1")).json() == set_replay(1, 0)
    assert (await admin_client.get(f"/matches/{match_id}/sets/3")).status_code == 404


async def test_priority_and_order(
    admin_client: AsyncClient, db: DBConnection, body: dict[str, Any]
) -> None:
    first = await enqueue(admin_client, body)
    urgent = await enqueue(admin_client, body, priority=1)
    second = await enqueue(admin_client, body)

    claimed = [await matches.claim_match(db, "w", LEASE) for _ in range(3)]
    assert [m.id for m in claimed if m is not None] == [urgent, first, second]


async def test_stale_lease_is_reclaimed(
    admin_client: AsyncClient, db: DBConnection, other: DBConnection, body: dict[str, Any]
) -> None:
    match_id = await enqueue(admin_client, body)
    held = await matches.claim_match(db, "w1", LEASE)
    assert held is not None
    assert await matches.heartbeat(db, held)

    await db.conn.execute(
        "update matches set heartbeat_at = now() - interval '1 hour' where id = %s", (match_id,)
    )
    taken = await matches.claim_match(other, "w2", LEASE)
    assert taken is not None and taken.id == match_id
    assert taken.attempts == 2
    assert taken.claimed_by == "w2"

    # The first holder can no longer write anything.
    assert not await matches.heartbeat(db, held)
    with pytest.raises(matches.LostLease):
        await matches.finish_match(db, held, replay(match_id))
    with pytest.raises(matches.LostLease):
        await matches.fail_match(db, held, "boom")
    await matches.requeue_match(db, held)
    match = await get(admin_client, match_id)
    assert match["status"] == "running"
    assert match["claimed_by"] == "w2"
    assert match["sets"] == []

    # The new holder finishes; sets from a partial earlier attempt are replaced.
    await other.match_repo.add_set(match_id, SetReplay.model_validate(set_replay(0, 1)))
    await matches.finish_match(other, taken, replay(match_id))
    match = await get(admin_client, match_id)
    assert match["status"] == "done"
    assert [s["winner_team"] for s in match["sets"]] == [None, 0, 0]


async def test_fail_and_requeue(
    admin_client: AsyncClient, db: DBConnection, body: dict[str, Any]
) -> None:
    match_id = await enqueue(admin_client, body)
    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None

    await matches.requeue_match(db, held)
    match = await get(admin_client, match_id)
    assert match["status"] == "queued"
    assert match["claimed_by"] is None
    assert match["attempts"] == 1

    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None
    await matches.fail_match(db, held, "ucbc run exited 2")
    match = await get(admin_client, match_id)
    assert match["status"] == "error"
    assert match["error"] == "ucbc run exited 2"
    assert match["completed_at"] is not None
    assert await matches.claim_match(db, "w", LEASE) is None


async def test_attempts_are_capped(
    admin_client: AsyncClient, db: DBConnection, body: dict[str, Any]
) -> None:
    exhausted = await enqueue(admin_client, body)
    fresh = await enqueue(admin_client, body)
    await db.conn.execute("update matches set attempts = 3 where id = %s", (exhausted,))

    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None and held.id == fresh
    match = await get(admin_client, exhausted)
    assert match["status"] == "error"
    assert match["error"] == "gave up after 3 attempts"


async def test_members_queue_and_see_their_own(
    admin_client: AsyncClient,
    member_client: AsyncClient,
    db: DBConnection,
    bot: Callable[[str], Path],
) -> None:
    mine = (await upload(member_client, zip_dir(bot("random")), form={"name": "mine"})).json()["id"]
    theirs = (
        await upload(admin_client, zip_dir(bot("first_empty")), form={"name": "theirs"})
    ).json()["id"]
    chess = (await upload(admin_client, zip_dir(bot("random")), form={"game": "chess"})).json()[
        "id"
    ]

    async def queue(bots: list[str], client: AsyncClient = member_client) -> Any:
        body = {"game": "tictactoe", "bots": bots, "config": {"seed": 1}, "priority": 5}
        return await client.post("/matches/queue", json=body)

    r = await queue([mine, theirs])
    assert r.status_code == 201, r.text
    match_id = r.json()["id"]
    match = (await member_client.get(f"/matches/{match_id}")).json()
    assert match["priority"] == 0
    assert [t["name"] for t in match["teams"]] == ["mine", "theirs"]
    assert match["bots"] == [mine, theirs]

    assert (await queue([theirs, theirs])).status_code == 403
    r = await queue([mine, chess])
    assert r.status_code == 400
    assert "is for chess" in r.json()["detail"]
    assert (await queue([mine, MISSING])).status_code == 404

    # A match without the member's submissions is invisible to them and absent from their list.
    admin_match = (await queue([theirs, theirs], admin_client)).json()["id"]
    assert (await member_client.get(f"/matches/{admin_match}")).status_code == 404
    assert (await member_client.get(f"/matches/{admin_match}/sets/0")).status_code == 404
    assert [m["id"] for m in (await member_client.get("/matches")).json()] == [match_id]
    assert [m["id"] for m in (await admin_client.get("/matches")).json()] == [
        admin_match,
        match_id,
    ]

    # A platform match is visible to everyone. Nothing creates one yet, so make it so.
    await db.conn.execute("update matches set origin = 'platform' where id = %s", (admin_match,))
    assert (await member_client.get(f"/matches/{admin_match}")).json()["origin"] == "platform"
    assert [m["id"] for m in (await member_client.get("/matches")).json()] == [
        admin_match,
        match_id,
    ]

    # `mine` drops the platform match: it is what the profile page lists.
    assert [m["id"] for m in (await member_client.get("/matches?mine=true")).json()] == [match_id]
    # An admin's own list is their own too, not every match. Both of these hold one of
    # their submissions: the member queued against it, which is what puts them in a match.
    assert [m["id"] for m in (await admin_client.get("/matches?mine=true")).json()] == [
        admin_match,
        match_id,
    ]


async def test_teams_share_matches(
    app: FastAPI,
    db: DBConnection,
    admin_client: AsyncClient,
    member_client: AsyncClient,
    teammate_client: AsyncClient,
    bot: Callable[[str], Path],
) -> None:
    code = zip_dir(bot("random"))

    async def uploaded(client: AsyncClient, name: str) -> str:
        r = await upload(client, code, form={"name": name})
        id: str = r.json()["id"]
        return id

    async def queue(client: AsyncClient, bots: list[str]) -> Any:
        body = {"game": "tictactoe", "bots": bots, "config": {"seed": 1}}
        return await client.post("/matches/queue", json=body)

    async def ids(client: AsyncClient, mine: bool = False) -> list[str]:
        r = await client.get("/matches", params={"mine": "true"} if mine else {})
        return [m["id"] for m in r.json()]

    alices = await uploaded(member_client, "alices")
    admins = await uploaded(admin_client, "admins")

    # Carol queues with her teammate's bot alone; the whole team sees the match.
    r = await queue(teammate_client, [alices, admins])
    assert r.status_code == 201, r.text
    match_id = r.json()["id"]
    for client in [member_client, teammate_client]:
        assert await ids(client) == [match_id]
        assert await ids(client, mine=True) == [match_id]
        assert (await client.get(f"/matches/{match_id}")).status_code == 200
        r = await client.get(f"/matches/{match_id}/sets/0")
        assert r.status_code == 404
        assert r.json()["detail"].startswith("no set")

    # Another team sees none of it and cannot queue with the team's bots alone.
    await create_user(db, "dave@example.com", "Dave", "dave-pw", is_admin=False)
    async with log_in(app, ("dave@example.com", "dave-pw")) as dave:
        assert await ids(dave) == []
        assert (await dave.get(f"/matches/{match_id}")).status_code == 404
        r = await dave.get(f"/matches/{match_id}/sets/0")
        assert r.json()["detail"].startswith("no match")
        r = await queue(dave, [alices, admins])
        assert r.status_code == 403
        assert "your team's submission" in r.json()["detail"]
        # With a bot of his own he may play against them.
        daves = await uploaded(dave, "daves")
        r = await queue(dave, [daves, alices])
        assert r.status_code == 201
        daves_match = r.json()["id"]
        assert await ids(dave) == [daves_match]
    # Alice's team is in Dave's match too; an admin sees both.
    assert await ids(member_client) == [daves_match, match_id]
    assert await ids(admin_client) == [daves_match, match_id]

    # Moving leaves the matches with the old team.
    assert (await member_client.post("/teams", json={"name": "Crabs"})).status_code == 201
    assert await ids(member_client, mine=True) == []
    assert (await member_client.get(f"/matches/{match_id}")).status_code == 404
    assert match_id in await ids(teammate_client, mine=True)


async def test_one_team_in_both_slots(
    member_client: AsyncClient, teammate_client: AsyncClient, bot: Callable[[str], Path]
) -> None:
    """Each slot is its own submission, even when one team, or one submission, fills both."""
    alices = (await upload(member_client, zip_dir(bot("random")), form={"name": "alices"})).json()
    carols = (
        await upload(teammate_client, zip_dir(bot("first_empty")), form={"name": "carols"})
    ).json()
    assert alices["team_id"] == carols["team_id"]

    for bots, names in [
        ([alices["id"], carols["id"]], ["alices", "carols"]),
        ([alices["id"], alices["id"]], ["alices", "alices"]),
    ]:
        body = {"game": "tictactoe", "bots": bots}
        r = await member_client.post("/matches/queue", json=body)
        assert r.status_code == 201, r.text
        match = (await teammate_client.get(f"/matches/{r.json()['id']}")).json()
        assert match["bots"] == bots
        assert match["teams"] == [{"id": i, "name": n} for i, n in enumerate(names)]
