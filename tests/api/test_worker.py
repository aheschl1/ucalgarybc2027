"""The worker plays real matches from the queue with `ucbc run`."""

import asyncio
from collections.abc import Callable
from pathlib import Path
from typing import Any
from uuid import UUID

import pytest
from fastapi import FastAPI
from httpx import AsyncClient

from api.blobs import BlobStore
from api.db import DBConnection
from api.services import matches
from api.services.maps import key_for
from tests.api.test_maps import MAPS
from tests.api.test_maps import upload as upload_map
from tests.api.test_queue import LEASE, get
from tests.api.test_submissions import upload, zip_dir
from worker import match as engine
from worker.main import Worker

BotPath = Callable[[str], Path]
UCBC2027_BOTS = Path(__file__).resolve().parents[2] / "bots" / "ucbc2027"


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


async def enqueue(
    client: AsyncClient,
    bots: list[Path],
    game: str = "tictactoe",
    sets: int = 3,
    maps: list[str] | None = None,
) -> UUID:
    """Uploads each bot directory as a submission of the client's team and queues them."""
    ids = []
    for b in bots:
        r = await upload(client, zip_dir(b), form={"name": b.name, "game": game})
        assert r.status_code == 201, r.text
        ids.append(r.json()["id"])
    body = {"game": game, "bots": ids, "config": {"seed": 7, "sets": sets}, "maps": maps or []}
    r = await client.post("/matches/queue", json=body)
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
    admin_client: AsyncClient, db: DBConnection, worker: Worker, bot: BotPath
) -> None:
    match_id = await enqueue(admin_client, [bot("random"), bot("first_empty")])

    assert await worker.run_once(db)
    match = await get(admin_client, match_id)
    assert match["status"] == "done", match["error"]
    assert [t["name"] for t in match["teams"]] == ["random", "first_empty"]
    assert match["attempts"] == 1
    assert match["engine_version"]
    assert [s["index"] for s in match["sets"]] == [0, 1, 2]
    assert sum(match["set_wins"]) == sum(1 for s in match["sets"] if s["winner_team"] is not None)
    replay = (await admin_client.get(f"/matches/{match_id}/sets/0")).json()
    assert {"detail": "", **replay["result"]} == match["sets"][0]

    assert not await worker.run_once(db)


async def test_worker_plays_a_map_per_set(
    app: FastAPI, admin_client: AsyncClient, db: DBConnection, worker: Worker
) -> None:
    ids = []
    for name in ["standard", "medium"]:
        r = await upload_map(admin_client, (MAPS / f"{name}.map").read_bytes(), f"{name}.map")
        ids.append(r.json()["id"])
    noop = UCBC2027_BOTS / "noop"
    match_id = await enqueue(admin_client, [noop, noop], "ucbc2027", sets=2, maps=ids)

    assert await worker.run_once(db)
    match = await get(admin_client, match_id)
    assert match["status"] == "done", match["error"]
    widths = []
    for index in [0, 1]:
        replay = (await admin_client.get(f"/matches/{match_id}/sets/{index}")).json()
        widths.append(len(replay["initial_state"]["environment"][0]))
    assert widths == [16, 32]


async def test_bad_maps_error_the_match(
    app: FastAPI, admin_client: AsyncClient, db: DBConnection, worker: Worker
) -> None:
    """The API records whatever maps a match names; playing it is where a bad pick shows,
    as the match's error."""
    standard = (await upload_map(admin_client)).json()["id"]
    broken = (await upload_map(admin_client, b"\xff\xff", "broken.map")).json()["id"]
    gone = (await upload_map(admin_client, filename="gone.map")).json()["id"]
    blobs: BlobStore = app.state.blobs
    await blobs.delete(key_for(UUID(gone)))
    noop = UCBC2027_BOTS / "noop"

    for sets, maps, error in [
        (1, [broken], "game_config.maps[0]: not a map file"),
        (3, [standard, standard], "once per set (3)"),
        (1, [gone], "missing from blob storage"),
    ]:
        match_id = await enqueue(admin_client, [noop, noop], "ucbc2027", sets=sets, maps=maps)
        assert await worker.run_once(db)
        match = await get(admin_client, match_id)
        assert match["status"] == "error"
        assert error in match["error"], match["error"]


async def test_unknown_game_errors_the_match(
    admin_client: AsyncClient, db: DBConnection, worker: Worker, bot: BotPath
) -> None:
    match_id = await enqueue(admin_client, [bot("random"), bot("random")], game="chess")

    assert await worker.run_once(db)
    match = await get(admin_client, match_id)
    assert match["status"] == "error"
    assert "chess" in match["error"]


async def test_start_failure_requeues(
    admin_client: AsyncClient,
    db: DBConnection,
    worker: Worker,
    bot: BotPath,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(engine, "UCBC", Path("/nonexistent/ucbc"))
    match_id = await enqueue(admin_client, [bot("random"), bot("random")])

    assert await worker.run_once(db)
    match = await get(admin_client, match_id)
    assert match["status"] == "queued"
    assert match["claimed_by"] is None
    assert match["attempts"] == 1


async def test_lost_lease_stops_the_engine(
    admin_client: AsyncClient,
    db: DBConnection,
    worker: Worker,
    bot: BotPath,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(engine, "command", lambda match, bots, maps, replay: ["sleep", "30"])
    match_id = await enqueue(admin_client, [bot("random"), bot("random")])

    running = asyncio.create_task(worker.run_once(db))
    await wait_for(admin_client, match_id, claimed_by="test")
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
    match = await get(admin_client, match_id)
    assert match["status"] == "running"
    assert match["claimed_by"] == "w2"
    assert match["attempts"] == 2


async def test_serve_runs_slots_and_drains(
    admin_client: AsyncClient, worker: Worker, bot: BotPath
) -> None:
    first = await enqueue(admin_client, [bot("random"), bot("first_empty")])
    second = await enqueue(admin_client, [bot("first_empty"), bot("random")])
    stop = asyncio.Event()

    serving = asyncio.create_task(worker.serve(stop))
    await wait_for(admin_client, first, status="done")
    await wait_for(admin_client, second, status="done")
    stop.set()
    await asyncio.wait_for(serving, 5)
