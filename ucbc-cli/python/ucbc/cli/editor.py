from pathlib import Path

import click


@click.command()
@click.argument("map_file", required=False, type=click.Path(dir_okay=False, path_type=Path))
@click.option("--port", default=0, show_default=True, help="0 picks a free port.")
@click.option("--no-browser", is_flag=True, help="Print the URL without opening a browser.")
def editor(map_file: Path | None, port: int, no_browser: bool) -> None:
    """Edit the ucbc2027 map MAP_FILE, served on localhost until Ctrl-C. Save writes it;
    if it does not exist yet, the editor starts on a new map. Without MAP_FILE, Save
    downloads the map through the browser."""
    from ucbc.editor import open_editor

    open_editor(map_file, port, browser=not no_browser)
