import click

from ucbc.cli.editor import editor
from ucbc.cli.login import login
from ucbc.cli.logout import logout
from ucbc.cli.run import run
from ucbc.cli.submit import submit
from ucbc.cli.view import view
from ucbc.cli.whoami import whoami


@click.group()
def main() -> None:
    """UCalgary Battlecode command line."""


main.add_command(run)
main.add_command(view)
main.add_command(editor)
main.add_command(submit)
main.add_command(login)
main.add_command(logout)
main.add_command(whoami)
