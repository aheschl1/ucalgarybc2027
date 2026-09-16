"""The worker plays real matches from the queue with `ucbc run`."""

import asyncio
from collections.abc import Callable
from pathlib import Path
from typing import Any
from uuid import UUID

import pytest
from fastapi import FastAPI
from httpx import AsyncClient

from api.db import DBConnection
from api.models.users import User
from api.services import matches
from tests.api.conftest import ADMIN, MEMBER
from tests.api.test_queue import LEASE, get
from tests.api.test_submissions import upload, zip_dir
from worker import match as engine
from worker.main import Worker

BotPath = Callable[[str], Path]


@pytest.fixture
def worker(app: FastAPI) -> Worker:
    return Worker(
        app.state.pool,
        app.state.blobs,
        name="test",
        slots=2,
        poll_s=0.05,
        heartbeat_s=0.1,
        lease=LEASE,
    )


async def enqueue(client: AsyncClient, bots: list[Path], **override: Any) -> UUID:
    body = {
        "game": "tictactoe",
        "bots": [{"kind": "path", "path": str(b)} for b in bots],
        "config": {"seed": 7},
        **override,
    }
    r = await client.post("/matches/queue", json=body, auth=ADMIN)
    assert r.status_code == 201, r.text
    return UUID(r.json()["id"])


async def wait_for(client: AsyncClient, match_id: UUID, **expected: Any) -> dict[str, Any]:
    for _ in range(200):
        match = await get(client, match_id)
        if all(match[k] == v for k, v in expected.items()):
            return match
        await asyncio.sleep(0.05)
    raise AssertionError(f"match {match_id} never reached {expected}: {match}")


async def test_worker_plays_a_queued_match(
    client: AsyncClient, admin: User, db: DBConnection, worker: Worker, bot: BotPath
) -> None:
    match_id = await enqueue(client, [bot("random"), bot("first_empty")])

    assert await worker.run_once(db)
    match = await get(client, match_id)
    assert match["status"] == "done"
    assert match["attempts"] == 1
    assert match["engine_version"]
    assert [s["index"] for s in match["sets"]] == [0, 1, 2]
    assert sum(match["set_wins"]) == sum(1 for s in match["sets"] if s["winner_team"] is not None)
    replay = (await client.get(f"/matches/{match_id}/sets/0", auth=ADMIN)).json()
    assert {"detail": "", **replay["result"]} == match["sets"][0]

    assert not await worker.run_once(db)


async def test_missing_bot_dir_errors_the_match(
    client: AsyncClient, admin: User, db: DBConnection, worker: Worker, bot: BotPath, tmp_path: Path
) -> None:
    match_id = await enqueue(client, [tmp_path / "missing", bot("random")])

    assert await worker.run_once(db)
    match = await get(client, match_id)
    assert match["status"] == "error"
    assert "does not exist" in match["error"]
    assert match["attempts"] == 1
    assert not await worker.run_once(db)


async def test_unknown_game_errors_the_match(
    client: AsyncClient, admin: User, db: DBConnection, worker: Worker, bot: BotPath
) -> None:
    match_id = await enqueue(client, [bot("random"), bot("random")], game="chess")

    assert await worker.run_once(db)
    match = await get(client, match_id)
    assert match["status"] == "error"
    assert "chess" in match["error"]


async def test_start_failure_requeues(
    client: AsyncClient,
    admin: User,
    db: DBConnection,
    worker: Worker,
    bot: BotPath,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(engine, "UCBC", Path("/nonexistent/ucbc"))
    match_id = await enqueue(client, [bot("random"), bot("random")])

    assert await worker.run_once(db)
    match = await get(client, match_id)
    assert match["status"] == "queued"
    assert match["claimed_by"] is None
    assert match["attempts"] == 1


async def test_lost_lease_stops_the_engine(
    client: AsyncClient,
    admin: User,
    db: DBConnection,
    worker: Worker,
    bot: BotPath,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(engine, "command", lambda match, bots, replay: ["sleep", "30"])
    match_id = await enqueue(client, [bot("random"), bot("random")])

    running = asyncio.create_task(worker.run_once(db))
    await wait_for(client, match_id, claimed_by="test")
    async with worker.pool.connection() as conn:
        await conn.set_autocommit(True)
        other = DBConnection(conn)
        await other.conn.execute(
            "update matches set heartbeat_at = now() - interval '1 hour' where id = %s",
            (match_id,),
        )
        taken = await matches.claim_match(other, "w2", LEASE)
    assert taken is not None

    # The first worker's heartbeat fails, it stops its engine, and it writes nothing.
    assert await asyncio.wait_for(running, 10)
    match = await get(client, match_id)
    assert match["status"] == "running"
    assert match["claimed_by"] == "w2"
    assert match["attempts"] == 2


async def test_worker_plays_uploaded_submissions(
    client: AsyncClient,
    admin: User,
    member: User,
    db: DBConnection,
    worker: Worker,
    bot: BotPath,
) -> None:
    mine = (await upload(client, zip_dir(bot("random")), form={"name": "mine"})).json()
    theirs = (
        await upload(client, zip_dir(bot("first_empty")), auth=ADMIN, form={"name": "theirs"})
    ).json()
    body = {
        "game": "tictactoe",
        "bots": [
            {"kind": "submission", "id": mine["id"]},
            {"kind": "submission", "id": theirs["id"]},
        ],
        "config": {"seed": 7},
    }
    r = await client.post("/matches/queue", json=body, auth=MEMBER)
    assert r.status_code == 201, r.text
    match_id = UUID(r.json()["id"])

    assert await worker.run_once(db)
    match = (await client.get(f"/matches/{match_id}", auth=MEMBER)).json()
    assert match["status"] == "done", match["error"]
    assert [t["name"] for t in match["teams"]] == ["mine", "theirs"]
    assert len(match["sets"]) == 3


async def test_serve_runs_slots_and_drains(
    client: AsyncClient, admin: User, worker: Worker, bot: BotPath
) -> None:
    first = await enqueue(client, [bot("random"), bot("first_empty")])
    second = await enqueue(client, [bot("first_empty"), bot("random")])
    stop = asyncio.Event()

    serving = asyncio.create_task(worker.serve(stop))
    await wait_for(client, first, status="done")
    await wait_for(client, second, status="done")
    stop.set()
    await asyncio.wait_for(serving, 5)
