import click
import httpx

from ucbc.cli.session import auth_headers, clear
from ucbc.settings import server_url
from uuid import UUID

@click.command()
@click.argument("mine", type=click.UUID)
@click.argument("opponent", type=click.UUID)
@click.option("--map", "maps", type=click.UUID, multiple=True, help="A map id. Once for every set, or once per set, in order.")
@click.option("--sets", type=click.IntRange(min=1), default=3, show_default=True)
@click.option("--seed", type=int, default=0, show_default=True)
def queue(mine: UUID, opponent: UUID, maps: tuple[UUID, ...], sets: int, seed: int) -> None:
    """Queue a match: your submission MINE against OPPONENT. See ids with 'ucbc submissions'."""    
    pass