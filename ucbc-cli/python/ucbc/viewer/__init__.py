"""Serves the replay viewer page and one replay file on localhost."""

import webbrowser
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import click

# Built by `make viewer` from ucbc-viewer/.
STATIC = Path(__file__).parent / "static"


def check_built() -> None:
    if not (STATIC / "index.html").is_file():
        raise click.ClickException("the viewer is not built; run `make viewer`")


def make_server(replay: Path, port: int = 0) -> ThreadingHTTPServer:
    """The page at `/`, and `replay` at `/replay.json`, which the page loads by default."""
    check_built()
    static = STATIC

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args: Any, **kwargs: Any) -> None:
            super().__init__(*args, directory=str(static), **kwargs)

        def translate_path(self, path: str) -> str:
            if path.split("?", 1)[0] == "/replay.json":
                return str(replay)
            return super().translate_path(path)

        def end_headers(self) -> None:
            # A rebuilt page or rewritten replay must not come from the browser cache.
            self.send_header("Cache-Control", "no-cache")
            super().end_headers()

        def log_message(self, format: str, *args: Any) -> None:
            pass

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


def open_viewer(replay: Path, port: int = 0, browser: bool = True) -> None:
    """Serve `replay` until Ctrl-C, opening it in the default browser."""
    try:
        server = make_server(replay.resolve(), port)
    except OSError as e:
        raise click.ClickException(f"cannot serve on port {port}: {e}") from e
    url = f"http://127.0.0.1:{server.server_address[1]}/"
    click.echo(f"Viewer: {url} (Ctrl-C to stop)")
    if browser:
        webbrowser.open(url)
    with server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
