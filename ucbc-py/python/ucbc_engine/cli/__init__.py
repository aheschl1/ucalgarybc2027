import click

from ucbc_engine.cli.run import run
from ucbc_engine.cli.view import view


@click.group()
def main() -> None:
    """UCalgary Battlecode command line."""


main.add_command(run)
main.add_command(view)
