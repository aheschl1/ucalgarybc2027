import click
import httpx

from ucbc.cli.session import auth_headers, clear
from ucbc.settings import server_url


@click.command()
def whoami() -> None:
    """Prints the current user"""
    headers = auth_headers()
    if not headers:
        raise click.ClickException("no user logged in: run ucbc login")
    try:
        response = httpx.get(f"{server_url()}/users/me", headers=headers)
    except httpx.HTTPError as e:
        raise click.ClickException(f"could not reach {server_url()}: {e}. Try again") from e
    if response.status_code == 401:
        clear()
        raise click.ClickException("session expired: log in again (run ucbc login)")
    if response.status_code != 200:
        raise click.ClickException(response.json().get("detail", response.text))

    body = response.json()
    click.echo(f"Logged in as {body['display_name']} ({body['email']})")
