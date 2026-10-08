import click
import httpx

from ucbc.cli.session import auth_headers, clear
from ucbc.settings import server_url


@click.command()
def logout() -> None:
    """logs out of the current user"""
    headers = auth_headers()
    if not headers:
        raise click.ClickException("no user logged in")
    try:
        response = httpx.post(f"{server_url()}/auth/logout", headers=headers)
    except httpx.HTTPError as e:
        raise click.ClickException(f"could not reach {server_url()}: {e}. Try again") from e
    if response.status_code != 204:
        raise click.ClickException(response.json().get("detail", response.text))
    clear()
    click.echo("Logged out successfully")
