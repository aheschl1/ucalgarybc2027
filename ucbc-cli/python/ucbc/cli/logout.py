from pathlib import Path

import click
import httpx

from ucbc.settings import server_url
from ucbc.cli.session import clear

@click.command()
def logout() -> None:
    """logs out of current user"""
    clear()