import click

from ucbc_engine.cli.run import run


@click.group()
def main() -> None:
    """UCalgary Battlecode command line."""


main.add_command(run)
