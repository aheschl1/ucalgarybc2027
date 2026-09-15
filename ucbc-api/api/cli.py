"""`ucbc-api-cli`: run the API's operations straight against the database.
`ENV=prod ucbc-api-cli ...` targets the production database through the same settings."""

import asyncio
from collections.abc import AsyncIterator, Awaitable, Callable
from contextlib import asynccontextmanager

import click
from psycopg import AsyncConnection
from psycopg.rows import DictRow, dict_row

from api.db import DBConnection
from api.services.users import UsernameTaken, create_user
from api.settings import settings


@asynccontextmanager
async def connect() -> AsyncIterator[DBConnection]:
    """One connection, committed at the end of the block."""
    conn: AsyncConnection[DictRow] = await AsyncConnection.connect(
        settings.database_url, row_factory=dict_row
    )
    async with conn:
        yield DBConnection(conn)


def run(command: Callable[[DBConnection], Awaitable[None]]) -> None:
    async def go() -> None:
        async with connect() as db:
            await command(db)

    try:
        asyncio.run(go())
    except UsernameTaken as e:
        raise click.ClickException(str(e)) from e


@click.group()
def main() -> None:
    """Manage the UCBC database directly."""


@main.command("create-user")
@click.option("--username", required=True)
@click.option("--password", required=True)
@click.option("--admin", is_flag=True)
def create_user_cmd(username: str, password: str, admin: bool) -> None:
    async def go(db: DBConnection) -> None:
        user = await create_user(db, username, password, admin)
        click.echo(f"created {user.username} (id {user.id}, admin={user.is_admin})")

    run(go)


@main.command("create-admin")
@click.option("--username", help="Defaults to UCBC_ADMIN_USERNAME.")
@click.option("--password", help="Defaults to UCBC_ADMIN_PASSWORD.")
def create_admin(username: str | None, password: str | None) -> None:
    username = username or settings.admin_username
    password = password or settings.admin_password
    if not username or not password:
        raise click.UsageError("set --username/--password or UCBC_ADMIN_USERNAME/PASSWORD")

    async def go(db: DBConnection) -> None:
        user = await create_user(db, username, password, is_admin=True)
        click.echo(f"created admin {user.username} (id {user.id})")

    run(go)


@main.command("list-users")
def list_users() -> None:
    async def go(db: DBConnection) -> None:
        for user in await db.user_repo.list():
            flag = " admin" if user.is_admin else ""
            click.echo(f"{user.id}\t{user.username}{flag}")

    run(go)
