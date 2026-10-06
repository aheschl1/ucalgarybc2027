"""`ucbc-api-cli`: admin operations through the API, logged in as an admin.
`--email`, `--password` and `--url` fall back to `UCBC_ADMIN_EMAIL`, `UCBC_ADMIN_PASSWORD`
and `UCBC_API_URL`, from the environment or the .env files."""

import asyncio
from pathlib import Path
from typing import Any

import click
import httpx
from pydantic_settings import BaseSettings, SettingsConfigDict

from api.env import env_files
from api.models.users import User
from api.services.sessions import COOKIE


class CliSettings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="UCBC_", env_file=env_files(), extra="ignore")

    admin_email: str | None = None
    admin_password: str | None = None
    api_url: str = "http://127.0.0.1:8000/api"


def check(r: httpx.Response) -> httpx.Response:
    if r.is_error:
        try:
            detail = r.json()["detail"]
        except (ValueError, KeyError, TypeError):
            detail = r.text
        raise click.ClickException(f"{r.status_code}: {detail}")
    return r


class Api:
    def __init__(self, url: str, email: str, password: str) -> None:
        self.url, self.email, self.password = url, email, password
        self._client: httpx.Client | None = None

    @property
    def client(self) -> httpx.Client:
        if self._client is None:
            client = httpx.Client(base_url=self.url.rstrip("/"), timeout=30)
            creds = {"email": self.email, "password": self.password}
            r = check(client.post("/auth/login", json=creds))
            # The cookie is Secure, which httpx will not send back over plain http.
            client.headers["Cookie"] = f"{COOKIE}={r.cookies[COOKIE]}"
            self._client = client
        return self._client

    def get(self, path: str, **kwargs: Any) -> Any:
        return check(self.client.get(path, **kwargs)).json()

    def post(self, path: str, **kwargs: Any) -> httpx.Response:
        return self.client.post(path, **kwargs)


def upload(api: Api, path: Path, game: str, name: str) -> httpx.Response:
    files = {"file": (path.name, path.read_bytes())}
    return api.post("/maps", files=files, data={"game": game, "name": name})


def setting(name: str) -> Any:
    return lambda: getattr(CliSettings(), name)


@click.group()
@click.option("--email", default=setting("admin_email"), required=True)
@click.option("--password", default=setting("admin_password"), required=True)
@click.option("--url", default=setting("api_url"), required=True)
@click.pass_context
def main(ctx: click.Context, email: str, password: str, url: str) -> None:
    """Manage UCBC as an admin."""
    ctx.obj = Api(url, email, password)


@main.command("create-user")
@click.argument("email")
@click.argument("display_name")
@click.option("--user-password", required=True, help="The new user's password.")
@click.option("--admin", is_flag=True)
@click.pass_obj
def create_user_cmd(
    api: Api, email: str, display_name: str, user_password: str, admin: bool
) -> None:
    body = {
        "email": email,
        "display_name": display_name,
        "password": user_password,
        "is_admin": admin,
    }
    user = check(api.post("/admin/users", json=body)).json()
    click.echo(f"created {user['email']} (id {user['id']}, admin={user['is_admin']})")


@main.command("create-admin")
@click.option("--display-name", default="admin", show_default=True)
@click.pass_obj
def create_admin(api: Api, display_name: str) -> None:
    """Create the --email/--password admin straight in the database, for the first one."""
    # Imported here: the database settings are needed by this command alone.
    from psycopg import AsyncConnection
    from psycopg.rows import dict_row

    from api.db import DBConnection
    from api.errors import ApiError
    from api.services.users import create_user
    from api.settings import settings

    async def go() -> User:
        conn = await AsyncConnection.connect(settings.database_url, row_factory=dict_row)
        async with conn:
            return await create_user(
                DBConnection(conn), api.email, display_name, api.password, is_admin=True
            )

    try:
        user = asyncio.run(go())
    except ApiError as e:
        raise click.ClickException(str(e)) from e
    click.echo(f"created admin {user.email} (id {user.id})")


@main.command("list-users")
@click.pass_obj
def list_users(api: Api) -> None:
    for user in api.get("/admin/users"):
        flag = " admin" if user["is_admin"] else ""
        click.echo(f"{user['id']}\t{user['email']}\t{user['display_name']}{flag}")


@main.command("delete-user")
@click.argument("user_id", type=int)
@click.pass_obj
def delete_user(api: Api, user_id: int) -> None:
    check(api.client.delete(f"/admin/users/{user_id}"))
    click.echo(f"deleted user {user_id}")


@main.command("upload-map")
@click.argument("file", type=click.Path(exists=True, dir_okay=False, path_type=Path))
@click.option("--game", required=True)
@click.option("--name", help="Defaults to the file's name.")
@click.pass_obj
def upload_map(api: Api, file: Path, game: str, name: str | None) -> None:
    m = check(upload(api, file, game, name or file.stem)).json()
    click.echo(f"uploaded {file.name} as {m['name']} (id {m['id']})")


@main.command("import-maps")
@click.argument("directory", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.option("--game", required=True)
@click.pass_obj
def import_maps(api: Api, directory: Path, game: str) -> None:
    """Upload every *.map file in DIRECTORY for GAME, named after the file. A name the
    game already has is skipped."""
    for path in sorted(directory.glob("*.map")):
        r = upload(api, path, game, path.stem)
        if r.status_code == 409:
            click.echo(f"skipped {path.name}: {r.json()['detail']}")
            continue
        m = check(r).json()
        click.echo(f"imported {path.name} as {m['name']} (id {m['id']})")
