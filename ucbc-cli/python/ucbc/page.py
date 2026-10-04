"""Serving a built page on localhost, as `ucbc view` and `ucbc editor` do."""

import webbrowser
from collections.abc import Callable
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import click


class PageHandler(SimpleHTTPRequestHandler):
    """Serves the files under `static`, uncached and unlogged. Subclasses set `static`."""

    static: Path

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, directory=str(self.static), **kwargs)

    def end_headers(self) -> None:
        # A rebuilt page or rewritten file must not come from the browser cache.
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    def log_message(self, format: str, *args: Any) -> None:
        pass


def check_built(static: Path, target: str) -> None:
    if not (static / "index.html").is_file():
        raise click.ClickException(f"the {target} is not built; run `make {target}`")


def serve(
    name: str, make_server: Callable[[int], ThreadingHTTPServer], port: int, browser: bool
) -> None:
    """Serve until Ctrl-C, opening the page in the default browser."""
    try:
        server = make_server(port)
    except OSError as e:
        raise click.ClickException(f"cannot serve on port {port}: {e}") from e
    url = f"http://127.0.0.1:{server.server_address[1]}/"
    click.echo(f"{name}: {url} (Ctrl-C to stop)")
    if browser:
        webbrowser.open(url)
    with server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
