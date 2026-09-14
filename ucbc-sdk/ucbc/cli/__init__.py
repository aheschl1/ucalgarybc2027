import click

from ucbc.cli.run import run


@click.group()
def main() -> None:
    """UCalgary Battlecode command line."""


main.add_command(run)
