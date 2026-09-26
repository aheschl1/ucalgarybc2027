"""Uploading bot code: checks on the zip, ownership, and the blob behind the row."""

import hashlib
import io
import zipfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

from fastapi import FastAPI
from httpx import AsyncClient

from api.blobs import BlobMissing, BlobStore
from api.db import DBConnection
from api.models.users import User
from api.services.submissions import MAX_ZIP, key_for

BotPath = Callable[[str], Path]


def zip_dir(directory: Path) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as zf:
        for path in sorted(directory.rglob("*")):
            if path.is_file():
                zf.write(path, path.relative_to(directory).as_posix())
    return buf.getvalue()


def zip_of(members: dict[str, bytes]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for name, data in members.items():
            zf.writestr(name, data)
    return buf.getvalue()


async def upload(client: AsyncClient, data: bytes, form: dict[str, str] | None = None) -> Any:
    return await client.post(
        "/submissions",
        files={"file": ("mybot.zip", data, "application/zip")},
        data={"game": "tictactoe", **(form or {})},
    )


async def test_upload_list_get(
    app: FastAPI,
    db: DBConnection,
    member: User,
    member_client: AsyncClient,
    admin_client: AsyncClient,
    bot: BotPath,
) -> None:
    data = zip_dir(bot("random"))
    r = await upload(member_client, data)
    assert r.status_code == 201, r.text
    body = r.json()
    assert body["name"] == "mybot"
    assert body["game"] == "tictactoe"
    assert body["display_name"] == "Alice"
    assert body["size"] == len(data)
    assert body["sha256"] == hashlib.sha256(data).hexdigest()

    # It belongs to the uploader's team.
    cur = await db.conn.execute("select team_id from submissions where id = %s", (body["id"],))
    assert await cur.fetchone() == {"team_id": member.team_id}

    r = await upload(admin_client, data, form={"name": "theirs"})
    assert r.status_code == 201
    assert r.json()["name"] == "theirs"

    r = await member_client.get("/submissions")
    assert [s["name"] for s in r.json()] == ["theirs", "mybot"]
    r = await member_client.get("/submissions", params={"mine": "true"})
    assert [s["name"] for s in r.json()] == ["mybot"]

    r = await admin_client.get(f"/submissions/{body['id']}")
    assert r.status_code == 200
    assert r.json() == body

    # The zip is in the bucket under the derived key.
    blobs: BlobStore = app.state.blobs
    assert await blobs.get(key_for(body["id"])) == data


async def test_rejected_uploads(
    client: AsyncClient, member_client: AsyncClient, bot: BotPath
) -> None:
    async def rejected(data: bytes, message: str, status: int = 400, **form: str) -> None:
        r = await upload(member_client, data, form=form)
        assert r.status_code == status, r.text
        assert message in r.json()["detail"]

    await rejected(zip_of({"bot.py": b""}), "main.py must be at the top")
    await rejected(zip_of({"sub/main.py": b""}), "main.py must be at the top")
    await rejected(zip_of({"main.py": b"", "../x.py": b""}), "bad path")
    await rejected(zip_of({"main.py": b"", "/etc/x": b""}), "bad path")
    await rejected(b"not a zip", "not a zip")
    await rejected(zip_of({"main.py": b"", "big": b"\0" * (9 << 20)}), "unpacks to more")
    await rejected(b"\0" * (MAX_ZIP + 1), "larger than", status=413)
    await rejected(zip_dir(bot("random")), "name must be", name="x" * 65)
    assert (
        await upload(member_client, zip_dir(bot("random")), form={"game": ""})
    ).status_code == 422

    assert (await client.post("/submissions", files={"file": b"x"})).status_code == 401
    assert (await client.get("/submissions")).status_code == 401
    missing = "00000000-0000-0000-0000-000000000000"
    assert (await member_client.get(f"/submissions/{missing}")).status_code == 404


async def test_blob_store_roundtrip(app: FastAPI) -> None:
    blobs: BlobStore = app.state.blobs
    await blobs.put("t/x", b"hello", "text/plain")
    assert await blobs.get("t/x") == b"hello"
    await blobs.delete("t/x")
    try:
        await blobs.get("t/x")
    except BlobMissing:
        pass
    else:
        raise AssertionError("deleted blob still readable")
    await blobs.ensure_bucket()
