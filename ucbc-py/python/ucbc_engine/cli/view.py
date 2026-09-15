from pathlib import Path

import click


@click.command()
@click.argument("replay", type=click.Path(exists=True, dir_okay=False, path_type=Path))
@click.option("--port", default=0, show_default=True, help="0 picks a free port.")
@click.option("--no-browser", is_flag=True, help="Print the URL without opening a browser.")
def view(replay: Path, port: int, no_browser: bool) -> None:
    """Open REPLAY in the viewer, served on localhost until Ctrl-C."""
    from ucbc_engine import viewer

    viewer.open_viewer(replay, port, browser=not no_browser)
