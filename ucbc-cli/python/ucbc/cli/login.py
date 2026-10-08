import click
import httpx

from ucbc.cli.session import COOKIE, save
from ucbc.settings import server_url


@click.command()
def login() -> None:
    """Log in with your email and password; successful login saves your session"""
    email = click.prompt("Email")
    password = click.prompt("Password", hide_input=True)
    body = {"email": email, "password": password}
    try:
        response = httpx.post(f"{server_url()}/auth/login", json=body)
    except httpx.HTTPError as e:
        raise click.ClickException(f"could not reach {server_url()}: {e}") from e
    if response.status_code == 401:
        raise click.ClickException("wrong password or email")
    if response.status_code == 429:
        raise click.ClickException("too many attempts, wait a minute")
    if response.status_code != 200:
        raise click.ClickException(response.json().get("detail", response.text))
    token = response.cookies.get(COOKIE)
    if token is None:
        raise click.ClickException("server error: try again")
    save(token)
    click.echo(f"Logged in successfully. Welcome, {response.json()['display_name']}")
