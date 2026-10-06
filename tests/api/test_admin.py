import asyncio
import socket
import threading
from collections.abc import Callable, Iterator
from pathlib import Path

import pytest
import uvicorn
from click.testing import CliRunner, Result
from fastapi import FastAPI
from httpx import AsyncClient
from pytest import MonkeyPatch

from api import cli, settings
from api.api import create_app
from api.db import DBConnection
from api.models.users import User
from tests.api.conftest import log_in
from tests.api.test_maps import MAPS
from tests.api.test_submissions import upload, zip_dir

NEW = {"email": "eve@example.com", "display_name": "Eve", "password": "eve-password"}


async def test_admin_users(
    app: FastAPI,
    admin: User,
    admin_client: AsyncClient,
    member: User,
    member_client: AsyncClient,
) -> None:
    assert (await member_client.get("/admin/users")).status_code == 403
    assert (await member_client.post("/admin/users", json=NEW)).status_code == 403
    assert (await member_client.delete(f"/admin/users/{admin.id}")).status_code == 403

    r = await admin_client.post("/admin/users", json={**NEW, "is_admin": True})
    assert r.status_code == 201, r.text
    eve = r.json()
    assert eve["is_admin"] is True
    r = await admin_client.get("/admin/users")
    assert [u["email"] for u in r.json()] == [admin.email, member.email, NEW["email"]]
    assert all("password_hash" not in u for u in r.json())

    async with log_in(app, (NEW["email"], NEW["password"])) as c:
        assert (await c.get("/users/me")).status_code == 200
        assert (await admin_client.delete(f"/admin/users/{eve['id']}")).status_code == 204
        # Her session went with her.
        assert (await c.get("/users/me")).status_code == 401
    assert (await admin_client.delete(f"/admin/users/{eve['id']}")).status_code == 404
    assert (await admin_client.delete(f"/admin/users/{admin.id}")).status_code == 400


async def test_cannot_delete_an_uploader(
    admin_client: AsyncClient,
    member: User,
    member_client: AsyncClient,
    bot: Callable[[str], Path],
) -> None:
    await upload(member_client, zip_dir(bot("random")))
    r = await admin_client.delete(f"/admin/users/{member.id}")
    assert r.status_code == 409
    assert "uploaded submissions" in r.json()["detail"]


@pytest.fixture
def server(database_url: str, blob_url: str) -> Iterator[str]:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    config = uvicorn.Config(
        create_app(database_url, blob_url), host="127.0.0.1", port=port, log_level="warning"
    )
    srv = uvicorn.Server(config)
    thread = threading.Thread(target=srv.run, daemon=True)
    thread.start()
    while not srv.started:
        threading.Event().wait(0.05)
    yield f"http://127.0.0.1:{port}"
    srv.should_exit = True
    thread.join()


async def test_cli(
    db: DBConnection,
    database_url: str,
    server: str,
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    monkeypatch.setattr(settings.settings, "database_url", database_url)
    monkeypatch.setenv("UCBC_ADMIN_EMAIL", "cli-admin@example.com")
    monkeypatch.setenv("UCBC_ADMIN_PASSWORD", "cli-password")
    monkeypatch.setenv("UCBC_API_URL", server)
    runner = CliRunner()

    async def invoke(*args: str) -> Result:
        # The CLI owns its own event loop, so it runs off the test's.
        return await asyncio.to_thread(runner.invoke, cli.main, list(args))

    async def ok(*args: str) -> str:
        result = await invoke(*args)
        assert result.exit_code == 0, result.output
        return result.output

    assert "created admin cli-admin@example.com" in await ok("create-admin")
    assert "already has an account" in (await invoke("create-admin")).output

    out = await ok("create-user", "eve@example.com", "Eve", "--user-password", "eve-password")
    assert "created eve@example.com" in out
    users = (await ok("list-users")).splitlines()
    assert users[0].endswith("cli-admin@example.com\tadmin admin")
    assert users[1].endswith("eve@example.com\tEve")
    assert "deleted user" in await ok("delete-user", users[1].split("\t")[0])

    out = await ok("upload-map", str(MAPS / "standard.map"), "--game", "ucbc2027", "--name", "s")
    assert "uploaded standard.map as s" in out
    (tmp_path / "s.map").write_bytes((MAPS / "medium.map").read_bytes())
    (tmp_path / "t.map").write_bytes((MAPS / "medium.map").read_bytes())
    out = await ok("import-maps", str(tmp_path), "--game", "ucbc2027")
    assert "skipped s.map" in out and "imported t.map as t" in out

    # A flag wins over the environment.
    result = await invoke("--password", "wrong", "list-users")
    assert result.exit_code != 0
    assert "401" in result.output
