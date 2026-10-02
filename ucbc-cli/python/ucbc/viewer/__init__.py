"""Serves the replay viewer page and one replay file on localhost."""

from http.server import ThreadingHTTPServer
from pathlib import Path

from ucbc import page

# Built by `make viewer` from ucbc-viewer/.
STATIC = Path(__file__).parent / "static"


def check_built() -> None:
    page.check_built(STATIC, "viewer")


def make_server(replay: Path, port: int = 0) -> ThreadingHTTPServer:
    """The page at `/`, and `replay` at `/replay.json`, which the page loads by default."""
    check_built()

    class Handler(page.PageHandler):
        static = STATIC

        def translate_path(self, path: str) -> str:
            if path.split("?", 1)[0] == "/replay.json":
                return str(replay)
            return super().translate_path(path)

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


def open_viewer(replay: Path, port: int = 0, browser: bool = True) -> None:
    """Serve `replay` until Ctrl-C, opening it in the default browser."""
    page.serve("Viewer", lambda p: make_server(replay.resolve(), p), port, browser)
