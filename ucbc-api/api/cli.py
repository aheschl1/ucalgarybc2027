"""`ucbc-api-cli`: run the API's operations straight against the database.
`ENV=prod ucbc-api-cli ...` targets the production database through the same settings."""

import asyncio
from collections.abc import AsyncIterator, Awaitable, Callable
from contextlib import asynccontextmanager

import click
from psycopg import AsyncConnection
from psycopg.rows import DictRow, dict_row

from api.db import DBConnection
from api.errors import ApiError
from api.services.users import create_user
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
    except ApiError as e:
        raise click.ClickException(str(e)) from e


@click.group()
def main() -> None:
    """Manage the UCBC database directly."""


@main.command("create-user")
@click.option("--email", required=True)
@click.option("--display-name", required=True)
@click.option("--password", required=True)
@click.option("--admin", is_flag=True)
def create_user_cmd(email: str, display_name: str, password: str, admin: bool) -> None:
    async def go(db: DBConnection) -> None:
        user = await create_user(db, email, display_name, password, admin)
        click.echo(f"created {user.email} (id {user.id}, admin={user.is_admin})")

    run(go)


@main.command("create-admin")
@click.option("--email", help="Defaults to UCBC_ADMIN_EMAIL.")
@click.option("--password", help="Defaults to UCBC_ADMIN_PASSWORD.")
@click.option("--display-name", default="admin", show_default=True)
def create_admin(email: str | None, password: str | None, display_name: str) -> None:
    email = email or settings.admin_email
    password = password or settings.admin_password
    if not email or not password:
        raise click.UsageError("set --email/--password or UCBC_ADMIN_EMAIL/PASSWORD")

    async def go(db: DBConnection) -> None:
        user = await create_user(db, email, display_name, password, is_admin=True)
        click.echo(f"created admin {user.email} (id {user.id})")

    run(go)


@main.command("list-users")
def list_users() -> None:
    async def go(db: DBConnection) -> None:
        for user in await db.user_repo.list():
            flag = " admin" if user.is_admin else ""
            click.echo(f"{user.id}\t{user.email}\t{user.display_name}{flag}")

    run(go)
