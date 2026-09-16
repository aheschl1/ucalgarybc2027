"""The match queue: enqueue and read through the API, claim and lease through the services."""

from collections.abc import AsyncIterator
from datetime import timedelta
from typing import Any
from uuid import UUID

import pytest
from fastapi import FastAPI
from httpx import AsyncClient

from api.db import DBConnection
from api.models.matches import MatchReplay, SetReplay
from api.models.users import User
from api.services import matches
from tests.api.conftest import ADMIN, MEMBER

LEASE = timedelta(seconds=30)
MISSING = "00000000-0000-0000-0000-000000000000"
REQUEST: dict[str, Any] = {
    "game": "tictactoe",
    "bots": [
        {"kind": "path", "path": "bots/tictactoe/random"},
        {"kind": "path", "path": "bots/tictactoe/first_empty"},
    ],
    "config": {"seed": 7},
}


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


async def enqueue(client: AsyncClient, **override: Any) -> UUID:
    r = await client.post("/matches/queue", json={**REQUEST, **override}, auth=ADMIN)
    assert r.status_code == 201, r.text
    return UUID(r.json()["id"])


async def get(client: AsyncClient, match_id: UUID) -> dict[str, Any]:
    r = await client.get(f"/matches/{match_id}", auth=ADMIN)
    assert r.status_code == 200, r.text
    body: dict[str, Any] = r.json()
    return body


async def test_enqueue_and_get(client: AsyncClient, admin: User) -> None:
    match_id = await enqueue(client)

    match = await get(client, match_id)
    assert match["status"] == "queued"
    assert match["engine_version"] is None
    assert match["bots"] == REQUEST["bots"]
    assert [t["name"] for t in match["teams"]] == ["random", "first_empty"]
    assert match["config"] == {"sets": 3, "seed": 7, "step_ms": 500, "memory_bytes": 2**30}
    assert match["attempts"] == 0
    assert match["claimed_by"] is None
    assert match["sets"] == []


async def test_permissions_and_validation(client: AsyncClient, admin: User, member: User) -> None:
    assert (await client.post("/matches/queue", json=REQUEST, auth=MEMBER)).status_code == 403
    one_bot = {**REQUEST, "bots": REQUEST["bots"][:1]}
    assert (await client.post("/matches/queue", json=one_bot, auth=ADMIN)).status_code == 422

    match_id = await enqueue(client)
    assert (await client.get(f"/matches/{match_id}")).status_code == 401
    assert (await client.get(f"/matches/{match_id}", auth=MEMBER)).status_code == 403
    assert (await client.get(f"/matches/{MISSING}", auth=ADMIN)).status_code == 404
    assert (await client.get(f"/matches/{match_id}/sets/0", auth=ADMIN)).status_code == 404


async def test_claim_is_exclusive(
    client: AsyncClient, admin: User, db: DBConnection, other: DBConnection
) -> None:
    match_id = await enqueue(client)

    held = await matches.claim_match(db, "w1", LEASE)
    assert held is not None and held.id == match_id
    assert held.status == "running"
    assert held.attempts == 1
    assert held.claimed_by == "w1"
    assert await matches.claim_match(other, "w2", LEASE) is None
    assert (await get(client, match_id))["status"] == "running"

    await matches.finish_match(db, held, replay(match_id))
    assert await matches.claim_match(other, "w2", LEASE) is None

    match = await get(client, match_id)
    assert match["status"] == "done"
    assert match["engine_version"] == "0.1.0"
    assert match["set_wins"] == [2, 0]
    assert match["winner_team"] == 0
    assert match["completed_at"] is not None
    expected = [{"detail": "", **set_replay(i, w)["result"]} for i, w in enumerate([None, 0, 0])]
    assert match["sets"] == expected

    # Set replays come back exactly as stored.
    r = await client.get(f"/matches/{match_id}/sets/0", auth=ADMIN)
    assert r.status_code == 200
    assert r.json() == set_replay(0, None)
    assert (await client.get(f"/matches/{match_id}/sets/1", auth=ADMIN)).json() == set_replay(1, 0)
    assert (await client.get(f"/matches/{match_id}/sets/3", auth=ADMIN)).status_code == 404


async def test_priority_and_order(client: AsyncClient, admin: User, db: DBConnection) -> None:
    first = await enqueue(client)
    urgent = await enqueue(client, priority=1)
    second = await enqueue(client)

    claimed = [await matches.claim_match(db, "w", LEASE) for _ in range(3)]
    assert [m.id for m in claimed if m is not None] == [urgent, first, second]


async def test_stale_lease_is_reclaimed(
    client: AsyncClient, admin: User, db: DBConnection, other: DBConnection
) -> None:
    match_id = await enqueue(client)
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
    match = await get(client, match_id)
    assert match["status"] == "running"
    assert match["claimed_by"] == "w2"
    assert match["sets"] == []

    # The new holder finishes; sets from a partial earlier attempt are replaced.
    await other.match_repo.add_set(match_id, SetReplay.model_validate(set_replay(0, 1)))
    await matches.finish_match(other, taken, replay(match_id))
    match = await get(client, match_id)
    assert match["status"] == "done"
    assert [s["winner_team"] for s in match["sets"]] == [None, 0, 0]


async def test_fail_and_requeue(client: AsyncClient, admin: User, db: DBConnection) -> None:
    match_id = await enqueue(client)
    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None

    await matches.requeue_match(db, held)
    match = await get(client, match_id)
    assert match["status"] == "queued"
    assert match["claimed_by"] is None
    assert match["attempts"] == 1

    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None
    await matches.fail_match(db, held, "ucbc run exited 2")
    match = await get(client, match_id)
    assert match["status"] == "error"
    assert match["error"] == "ucbc run exited 2"
    assert match["completed_at"] is not None
    assert await matches.claim_match(db, "w", LEASE) is None


async def test_attempts_are_capped(client: AsyncClient, admin: User, db: DBConnection) -> None:
    exhausted = await enqueue(client)
    fresh = await enqueue(client)
    await db.conn.execute("update matches set attempts = 3 where id = %s", (exhausted,))

    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None and held.id == fresh
    match = await get(client, exhausted)
    assert match["status"] == "error"
    assert match["error"] == "gave up after 3 attempts"
