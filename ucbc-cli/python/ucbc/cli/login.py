from pathlib import Path

import click
import httpx

from ucbc.settings import server_url, https_check
from ucbc.cli.session import COOKIE, save
@click.command()
def login() -> None:
    """Log in"""
    email = click.prompt("Email")
    password = click.prompt("Password", hide_input=True)
    body = {"email": email, "password": password}
    try:
        response = httpx.post(
            f"{server_url()}/auth/login",
            json=body
        )
    except httpx.HTTPError as e:
        raise click.ClickException(f"could not reach {server_url()}: {e}") from e
    if response.status_code == 401:
        raise click.ClickException("wrong password or email")    
    if response.status_code == 429:
        raise click.ClickException("too many attempts, wait a minute")
    if response.status_code != 200:
        raise click.ClickException(response.json().get("detail", response.text))
    token = response.cookies[COOKIE]
    if token is None:
        raise click.ClickException("server error: try again")
    save(token)
    username = response.json()['display_name']
    click.echo(f"Logged in successfully. Welcome {username}")
    