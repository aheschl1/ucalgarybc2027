"""Serves the map editor page on localhost, editing one map file or none."""

from http.server import ThreadingHTTPServer
from pathlib import Path

from ucbc import page

# Built by `make editor` from games/ucbc2027/editor/.
STATIC = Path(__file__).parent / "static"
MAP = "/map.bin"


def check_built() -> None:
    page.check_built(STATIC, "editor")


def make_server(map_file: Path | None, port: int = 0) -> ThreadingHTTPServer:
    """The page at `/`, and `map_file` at `/map.bin`: GET reads it (404 until it exists,
    which the page takes as a new map), PUT replaces it. Without a file both are 404, and
    the page downloads the map instead."""
    check_built()

    class Handler(page.PageHandler):
        static = STATIC

        def translate_path(self, path: str) -> str:
            if map_file is not None and path.split("?", 1)[0] == MAP:
                return str(map_file)
            return super().translate_path(path)

        def do_PUT(self) -> None:
            if map_file is None or self.path.split("?", 1)[0] != MAP:
                self.send_error(404)
                return
            body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
            # Written beside it, then swapped in, so a failed write leaves the old file.
            partial = map_file.with_name(map_file.name + ".partial")
            partial.write_bytes(body)
            partial.replace(map_file)
            self.send_response(204)
            self.end_headers()

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


def open_editor(map_file: Path | None, port: int = 0, browser: bool = True) -> None:
    """Serve `map_file`, or a new map, in the editor until Ctrl-C, opening it in the
    default browser."""
    resolved = map_file and map_file.resolve()
    page.serve("Editor", lambda p: make_server(resolved, p), port, browser)
