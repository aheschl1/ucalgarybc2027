from pathlib import Path

import click
import httpx

from ucbc.cli.session import auth_headers, clear
from ucbc.runner import default_game
from ucbc.settings import server_url

@click.command()
@click.argument("USER")
def submissions():
    pass