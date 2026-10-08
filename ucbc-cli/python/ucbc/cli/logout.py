import click

from ucbc.cli.session import clear


@click.command()
def logout() -> None:
    """Logs out of current user"""
    clear()
    click.echo("Logged out successfully")
