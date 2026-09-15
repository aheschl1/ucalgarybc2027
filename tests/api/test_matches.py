from typing import Any

from httpx import AsyncClient

from api.models import User
from tests.api.conftest import ADMIN, MEMBER

MATCH = {
    "game": "tictactoe",
    "engine_version": "0.1.0",
    "teams": [{"id": 0, "name": "random"}, {"id": 1, "name": "first_empty"}],
    "config": {"sets": 3, "seed": 7, "step_ms": 500, "memory_bytes": 2**30},
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


RESULT: dict[str, Any] = {
    "sets": [set_replay(i, w)["result"] for i, w in enumerate([None, 0, 0])],
    "set_wins": [2, 0],
    "winner_team": 0,
}


async def create(client: AsyncClient) -> str:
    r = await client.post("/matches", json=MATCH, auth=ADMIN)
    assert r.status_code == 201, r.text
    id: str = r.json()["id"]
    return id


async def test_match_lifecycle(client: AsyncClient, admin: User) -> None:
    id = await create(client)

    r = await client.get(f"/matches/{id}", auth=ADMIN)
    assert r.status_code == 200
    assert r.json()["status"] == "running"
    assert r.json()["sets"] == []

    for i, winner in enumerate([None, 0, 0]):
        r = await client.post(f"/matches/{id}/sets", json=set_replay(i, winner), auth=ADMIN)
        assert r.status_code == 204, r.text

    r = await client.post(f"/matches/{id}/complete", json=RESULT, auth=ADMIN)
    assert r.status_code == 204, r.text

    body = (await client.get(f"/matches/{id}", auth=ADMIN)).json()
    assert body["status"] == "done"
    assert body["set_wins"] == [2, 0]
    assert body["winner_team"] == 0
    assert body["completed_at"] is not None
    assert body["teams"] == MATCH["teams"]
    assert body["config"] == MATCH["config"]
    assert body["sets"] == [{"detail": "", **s} for s in RESULT["sets"]]

    r = await client.get(f"/matches/{id}/sets/0", auth=ADMIN)
    assert r.status_code == 200
    assert r.json() == set_replay(0, None)
    assert (await client.get(f"/matches/{id}/sets/3", auth=ADMIN)).status_code == 404


async def test_conflicts(client: AsyncClient, admin: User) -> None:
    id = await create(client)
    assert (
        await client.post(f"/matches/{id}/sets", json=set_replay(0, 0), auth=ADMIN)
    ).status_code == 204
    r = await client.post(f"/matches/{id}/sets", json=set_replay(0, 0), auth=ADMIN)
    assert r.status_code == 409
    assert "already has set 0" in r.json()["detail"]

    # One set stored, result names three.
    r = await client.post(f"/matches/{id}/complete", json=RESULT, auth=ADMIN)
    assert r.status_code == 409

    one = {"sets": [set_replay(0, 0)["result"]], "set_wins": [1, 0], "winner_team": 0}
    assert (await client.post(f"/matches/{id}/complete", json=one, auth=ADMIN)).status_code == 204
    assert (await client.post(f"/matches/{id}/complete", json=one, auth=ADMIN)).status_code == 409
    r = await client.post(f"/matches/{id}/sets", json=set_replay(1, 0), auth=ADMIN)
    assert r.status_code == 409
    assert "is done" in r.json()["detail"]


async def test_unknown_match_and_permissions(
    client: AsyncClient, admin: User, member: User
) -> None:
    missing = "00000000-0000-0000-0000-000000000000"
    assert (await client.get(f"/matches/{missing}", auth=ADMIN)).status_code == 404
    assert (
        await client.post(f"/matches/{missing}/sets", json=set_replay(0, 0), auth=ADMIN)
    ).status_code == 404
    assert (await client.post("/matches", json=MATCH, auth=MEMBER)).status_code == 403
    id = await create(client)
    assert (
        await client.post(f"/matches/{id}/sets", json=set_replay(0, 0), auth=MEMBER)
    ).status_code == 403
    assert (await client.get(f"/matches/{id}")).status_code == 401
