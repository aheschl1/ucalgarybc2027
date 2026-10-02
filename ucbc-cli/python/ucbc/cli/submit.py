from pathlib import Path

import click

@click.command()
@click.argument("bot", type=click.Path(exists=True, file_okay=False, path_type=Path)) # makes sure the input is a directory, not a file
@click.option("--game", required=True, type=click.Choice(["tictactoe", "ucbc2027"]))
@click.option("--name", help="Defaults to folder name")

def submit(bot: Path, game: str, name: str | None) -> None:
    """Zip BOT (a directory with main.py at its root) and submit it."""
    if not (bot / "main.py").is_file():
        raise click.ClickException(f"{bot} has no main.py")
        