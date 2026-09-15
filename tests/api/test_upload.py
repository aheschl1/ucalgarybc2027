"""A real match played with `upload=True` against the API served by uvicorn."""

import asyncio
import json
from collections.abc import AsyncIterator, Callable
from pathlib import Path

import pytest
import uvicorn
from fastapi import FastAPI
from httpx import AsyncClient
from ucbc_engine.runner import run_match
from ucbc_engine.upload import Uploader, UploadError

from api.models import User
from tests.api.conftest import ADMIN

BotPath = Callable[[str], Path]


@pytest.fixture
async def server(app: FastAPI) -> AsyncIterator[str]:
    # The app fixture already ran the lifespan, so the pool exists.
    server = uvicorn.Server(
        uvicorn.Config(app, host="127.0.0.1", port=0, lifespan="off", log_level="warning")
    )
    task = asyncio.create_task(server.serve())
    while not server.started:
        await asyncio.sleep(0.02)
    port = server.servers[0].sockets[0].getsockname()[1]
    yield f"http://127.0.0.1:{port}"
    server.should_exit = True
    await task


@pytest.fixture
def credentials(server: str, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("UCBC_API_URL", server)
    monkeypatch.setenv("UCBC_API_USERNAME", ADMIN[0])
    monkeypatch.setenv("UCBC_API_PASSWORD", ADMIN[1])


async def test_match_is_recorded_set_by_set(
    client: AsyncClient, admin: User, credentials: None, bot: BotPath, tmp_path: Path
) -> None:
    replay_path = tmp_path / "replay.json"
    # The server shares this event loop, so the match runs in a thread.
    result = await asyncio.to_thread(
        run_match, bot("random"), bot("first_empty"), seed=7, replay_path=replay_path, upload=True
    )
    replay = json.loads(replay_path.read_text())
    match_id = replay["match_id"]
    assert match_id != "local"

    r = await client.get(f"/matches/{match_id}", auth=ADMIN)
    assert r.status_code == 200
    body = r.json()
    assert body["status"] == "done"
    assert body["game"] == "tictactoe"
    assert body["engine_version"] == replay["engine_version"]
    assert [t["name"] for t in body["teams"]] == ["random", "first_empty"]
    assert body["config"] == {"sets": 3, "seed": 7, "step_ms": 500, "memory_bytes": 2**30}
    assert body["set_wins"] == result["set_wins"]
    assert body["winner_team"] == result["winner_team"]
    assert [s["index"] for s in body["sets"]] == [0, 1, 2]

    r = await client.get(f"/matches/{match_id}/sets/0", auth=ADMIN)
    assert r.json() == replay["sets"][0]


async def test_wrong_password_fails_the_upload(
    admin: User, credentials: None, bot: BotPath, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("UCBC_API_PASSWORD", "wrong")
    # Creating the match fails, so the match plays without uploading.
    result = await asyncio.to_thread(run_match, bot("random"), bot("random"), sets=1, upload=True)
    assert len(result["sets"]) == 1


async def test_set_upload_failure_aborts_the_match(
    client: AsyncClient,
    admin: User,
    credentials: None,
    bot: BotPath,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail(self: Uploader, match_id: str, set_json: str) -> None:
        raise UploadError("api down")

    monkeypatch.setattr(Uploader, "add_set", fail)
    with pytest.raises(UploadError, match="api down"):
        await asyncio.to_thread(run_match, bot("random"), bot("random"), sets=1, upload=True)
