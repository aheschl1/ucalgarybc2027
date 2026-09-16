import asyncio

from click.testing import CliRunner, Result
from fastapi import FastAPI
from httpx import AsyncClient
from pytest import MonkeyPatch

from api import cli, settings
from tests.api.conftest import MEMBER, log_in


async def test_health(client: AsyncClient) -> None:
    r = await client.get("/health")
    assert r.status_code == 200
    assert r.json() == {"status": "ok"}


async def test_me(client: AsyncClient, member_client: AsyncClient) -> None:
    assert (await client.get("/users/me")).status_code == 401
    r = await member_client.get("/users/me")
    assert r.status_code == 200
    body = r.json()
    assert body["username"] == MEMBER[0]
    assert body["is_admin"] is False
    assert "password_hash" not in body


async def test_create_user_is_admin_only(
    app: FastAPI, admin_client: AsyncClient, member_client: AsyncClient
) -> None:
    body = {"username": "bob", "password": "bob-pw"}
    assert (await member_client.post("/users", json=body)).status_code == 403
    r = await admin_client.post("/users", json=body)
    assert r.status_code == 201
    assert r.json()["username"] == "bob"
    async with log_in(app, ("bob", "bob-pw")) as bob:
        assert (await bob.get("/users/me")).status_code == 200
    assert (await admin_client.post("/users", json=body)).status_code == 409


async def test_cli_create_admin_can_log_in(
    app: FastAPI, database_url: str, monkeypatch: MonkeyPatch
) -> None:
    monkeypatch.setattr(settings.settings, "database_url", database_url)
    monkeypatch.setattr(settings.settings, "admin_username", "cli-admin")
    monkeypatch.setattr(settings.settings, "admin_password", "cli-pw")
    runner = CliRunner()

    async def invoke(*args: str) -> Result:
        # The CLI owns its own event loop, so it runs off the test's.
        return await asyncio.to_thread(runner.invoke, cli.main, list(args))

    result = await invoke("create-admin")
    assert result.exit_code == 0, result.output
    assert "created admin cli-admin" in result.output

    async with log_in(app, ("cli-admin", "cli-pw")) as c:
        r = await c.get("/users/me")
    assert r.status_code == 200
    assert r.json()["is_admin"] is True

    result = await invoke("create-admin")
    assert result.exit_code != 0
    assert "taken" in result.output

    result = await invoke("list-users")
    assert result.output.strip().endswith("cli-admin admin")
