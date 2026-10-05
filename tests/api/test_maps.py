"""Map files: admin upload, the listing, archiving, and the maps a queued match names. The
API never reads a map file; tests/api/test_worker.py has what a bad one does to a match."""

import hashlib
from collections.abc import Callable
from pathlib import Path
from typing import Any

from fastapi import FastAPI
from httpx import AsyncClient

from api.blobs import BlobStore
from api.services.maps import MAX_MAP, key_for
from tests.api.test_submissions import upload as upload_bot
from tests.api.test_submissions import zip_dir

MAPS = Path(__file__).resolve().parents[2] / "games" / "ucbc2027" / "maps"
STANDARD = (MAPS / "standard.map").read_bytes()
MISSING = "00000000-0000-0000-0000-000000000000"


async def upload(
    client: AsyncClient, data: bytes = STANDARD, filename: str = "standard.map", **form: str
) -> Any:
    return await client.post(
        "/maps",
        files={"file": (filename, data, "application/octet-stream")},
        data={"game": "ucbc2027", **form},
    )


async def test_upload_list_get(
    app: FastAPI, admin_client: AsyncClient, member_client: AsyncClient
) -> None:
    r = await upload(admin_client)
    assert r.status_code == 201, r.text
    body = r.json()
    assert (body["name"], body["game"], body["size"]) == ("standard", "ucbc2027", len(STANDARD))
    assert body["sha256"] == hashlib.sha256(STANDARD).hexdigest()
    assert body["archived_at"] is None

    medium = (MAPS / "medium.map").read_bytes()
    r = await upload(admin_client, medium, "medium.map", name="Arena")
    assert r.status_code == 201, r.text
    assert r.json()["name"] == "Arena"

    # Members read the metadata; only admins the file.
    r = await member_client.get("/maps", params={"game": "ucbc2027"})
    assert [m["name"] for m in r.json()] == ["Arena", "standard"]
    assert (await member_client.get("/maps", params={"game": "tictactoe"})).json() == []
    assert (await member_client.get(f"/maps/{body['id']}")).json() == body
    assert (await member_client.get(f"/maps/{body['id']}/file")).status_code == 403
    r = await admin_client.get(f"/maps/{body['id']}/file")
    assert r.status_code == 200
    assert r.content == STANDARD

    blobs: BlobStore = app.state.blobs
    assert await blobs.get(key_for(body["id"])) == STANDARD


async def test_rejected_uploads(
    client: AsyncClient, admin_client: AsyncClient, member_client: AsyncClient
) -> None:
    async def rejected(data: bytes, message: str, status: int = 400, **form: str) -> None:
        r = await upload(admin_client, data, **form)
        assert r.status_code == status, r.text
        assert message in r.json()["detail"]

    await rejected(b"\0" * (MAX_MAP + 1), "larger than", status=413)
    await rejected(STANDARD, "name must be", name="x" * 65)

    assert (await upload(admin_client)).status_code == 201
    await rejected(STANDARD, "already has a map named", status=409, name="STANDARD")
    # The same name is free in another game.
    assert (await upload(admin_client, game="tictactoe")).status_code == 201

    assert (await upload(member_client)).status_code == 403
    assert (await upload(client)).status_code == 401
    assert (await client.get("/maps")).status_code == 401
    assert (await member_client.get(f"/maps/{MISSING}")).status_code == 404


async def test_archive(admin_client: AsyncClient, member_client: AsyncClient) -> None:
    id = (await upload(admin_client)).json()["id"]
    assert (await member_client.patch(f"/maps/{id}", json={"archived": True})).status_code == 403

    r = await admin_client.patch(f"/maps/{id}", json={"archived": True})
    assert r.status_code == 200
    archived_at = r.json()["archived_at"]
    assert archived_at is not None
    # Archiving again keeps the first time; the listing still shows it.
    r = await admin_client.patch(f"/maps/{id}", json={"archived": True})
    assert r.json()["archived_at"] == archived_at
    assert [m["id"] for m in (await member_client.get("/maps")).json()] == [id]

    r = await admin_client.patch(f"/maps/{id}", json={"archived": False})
    assert r.json()["archived_at"] is None
    assert (
        await admin_client.patch(f"/maps/{MISSING}", json={"archived": True})
    ).status_code == 404


async def test_queue_with_maps(
    admin_client: AsyncClient, member_client: AsyncClient, bot: Callable[[str], Path]
) -> None:
    standard = (await upload(admin_client)).json()["id"]
    medium = (await upload(admin_client, (MAPS / "medium.map").read_bytes(), "medium.map")).json()
    other = (await upload(admin_client, game="tictactoe")).json()["id"]
    r = await upload_bot(member_client, zip_dir(bot("random")), form={"game": "ucbc2027"})
    bots = [r.json()["id"]] * 2

    async def queue(maps: list[str], sets: int = 3) -> Any:
        body = {"game": "ucbc2027", "bots": bots, "config": {"sets": sets}, "maps": maps}
        return await member_client.post("/matches/queue", json=body)

    # A member picks any maps; they are recorded as given, unread, and the worker reports
    # a bad pick as the match's error.
    for maps in [[], [standard], [standard, medium["id"], standard], [other], [MISSING]]:
        r = await queue(maps)
        assert r.status_code == 201, r.text
        match = (await member_client.get(f"/matches/{r.json()['id']}")).json()
        assert match["maps"] == maps
