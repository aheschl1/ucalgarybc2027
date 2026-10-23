from uuid import UUID

import click
import httpx

from ucbc.cli.session import auth_headers, check_status
from ucbc.settings import server_url


@click.command()
@click.argument("mine", type=click.UUID)
@click.argument("opponent", type=click.UUID)
@click.option(
    "--map",
    "maps",
    type=click.UUID,
    multiple=True,
    help="A map id. Once for every set, or once per set, in order.",
)
@click.option("--sets", type=click.IntRange(min=1), default=3, show_default=True)
@click.option("--seed", type=int, default=0, show_default=True)
def queue(mine: UUID, opponent: UUID, maps: tuple[UUID, ...], sets: int, seed: int) -> None:
    """Queue a match: your submission MINE against OPPONENT. See ids with 'ucbc submissions'."""
    if maps and len(maps) not in (1, sets):
        raise click.UsageError(
            f"--map was given {len(maps)} times; give it once, or once per set ({sets})"
        )

    headers = auth_headers()
    if not headers:
        raise click.ClickException("no user logged in: run ucbc login")
    try:
        response = httpx.get(f"{server_url()}/submissions/{mine}", headers=headers)
        check_status(response)

        game = response.json()["game"]
        response = httpx.post(
            f"{server_url()}/matches/queue",
            headers=headers,
            json={
                "game": game,
                "bots": [str(mine), str(opponent)],
                "config": {"sets": sets, "seed": seed},
                "maps": [str(m) for m in maps],
            },
        )
        check_status(response)
    except httpx.HTTPError as e:
        raise click.ClickException(f"could not reach {server_url()}: {e}") from e
    body = response.json()
    click.echo(f"Queued match ({body['id']})")
