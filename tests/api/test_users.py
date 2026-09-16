import asyncio

from click.testing import CliRunner, Result
from httpx import AsyncClient
from pytest import MonkeyPatch

from api import cli, settings
from api.models.users import User
from tests.api.conftest import ADMIN, MEMBER


async def test_health(client: AsyncClient) -> None:
    r = await client.get("/health")
    assert r.status_code == 200
    assert r.json() == {"status": "ok"}


async def test_me_requires_valid_credentials(client: AsyncClient, member: User) -> None:
    assert (await client.get("/users/me")).status_code == 401
    r = await client.get("/users/me", auth=(MEMBER[0], "wrong"))
    assert r.status_code == 401
    assert "www-authenticate" not in r.headers
    r = await client.get("/users/me", auth=MEMBER)
    assert r.status_code == 200
    body = r.json()
    assert body["username"] == MEMBER[0]
    assert body["is_admin"] is False
    assert "password_hash" not in body


async def test_create_user_is_admin_only(client: AsyncClient, admin: User, member: User) -> None:
    body = {"username": "bob", "password": "bob-pw"}
    assert (await client.post("/users", json=body, auth=MEMBER)).status_code == 403
    r = await client.post("/users", json=body, auth=ADMIN)
    assert r.status_code == 201
    assert r.json()["username"] == "bob"
    assert (await client.get("/users/me", auth=("bob", "bob-pw"))).status_code == 200
    assert (await client.post("/users", json=body, auth=ADMIN)).status_code == 409


async def test_cli_create_admin_can_log_in(
    client: AsyncClient, database_url: str, monkeypatch: MonkeyPatch
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

    r = await client.get("/users/me", auth=("cli-admin", "cli-pw"))
    assert r.status_code == 200
    assert r.json()["is_admin"] is True

    result = await invoke("create-admin")
    assert result.exit_code != 0
    assert "taken" in result.output

    result = await invoke("list-users")
    assert result.output.strip().endswith("cli-admin admin")
