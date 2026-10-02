"""Serves the replay viewer page and one replay file on localhost."""

from http.server import ThreadingHTTPServer
from pathlib import Path
from typing import BinaryIO

from ucbc import page

# Built by `make viewer` from ucbc-viewer/.
STATIC = Path(__file__).parent / "static"
GZIP_MAGIC = b"\x1f\x8b"


def check_built() -> None:
    page.check_built(STATIC, "viewer")


def make_server(replay: Path, port: int = 0) -> ThreadingHTTPServer:
    """The page at `/`, and `replay` at `/replay.json`, which the page loads by default.
    A gzipped replay is sent with `Content-Encoding: gzip`."""
    check_built()

    class Handler(page.PageHandler):
        static = STATIC

        def send_head(self) -> BinaryIO | None:
            if self.path.split("?", 1)[0] != "/replay.json":
                return super().send_head()
            try:
                f = replay.open("rb")
            except OSError:
                self.send_error(404)
                return None
            # A gzipped replay goes as it is; the browser decompresses it.
            gzipped = f.read(2) == GZIP_MAGIC
            f.seek(0)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            if gzipped:
                self.send_header("Content-Encoding", "gzip")
            self.send_header("Content-Length", str(replay.stat().st_size))
            self.end_headers()
            return f

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


def open_viewer(replay: Path, port: int = 0, browser: bool = True) -> None:
    """Serve `replay` until Ctrl-C, opening it in the default browser."""
    page.serve("Viewer", lambda p: make_server(replay.resolve(), p), port, browser)
