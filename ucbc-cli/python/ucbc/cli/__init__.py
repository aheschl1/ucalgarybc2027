import click

from ucbc.cli.run import run
from ucbc.cli.view import view
from ucbc.cli.submit import submit

@click.group()
def main() -> None:
    """UCalgary Battlecode command line."""


main.add_command(run)
main.add_command(view)
main.add_command(submit)
