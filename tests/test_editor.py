import threading
import urllib.error
import urllib.request
from collections.abc import Iterator
from pathlib import Path

import click
import pytest
from click.testing import CliRunner
from ucbc import editor
from ucbc.cli import main

ROOT = Path(__file__).resolve().parents[1]
STANDARD = ROOT / "games" / "ucbc2027" / "maps" / "standard.map"


@pytest.fixture
def static(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A stand-in for the built page, so these tests do not need Node."""
    page = tmp_path / "static"
    page.mkdir()
    (page / "index.html").write_text("<title>editor</title>")
    monkeypatch.setattr(editor, "STATIC", page)
    return page


@pytest.fixture
def map_file(tmp_path: Path) -> Path:
    return tmp_path / "arena.map"


@pytest.fixture
def served(static: Path, map_file: Path) -> Iterator[str]:
    server = editor.make_server(map_file)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{server.server_address[1]}"
    server.shutdown()
    server.server_close()


def request(url: str, method: str = "GET", body: bytes | None = None) -> tuple[int, bytes]:
    try:
        with urllib.request.urlopen(urllib.request.Request(url, body, method=method)) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, b""


def test_serves_the_page_and_the_map_file(served: str, map_file: Path) -> None:
    assert request(f"{served}/") == (200, b"<title>editor</title>")
    # Not there yet: the page starts a new map.
    assert request(f"{served}/map.bin")[0] == 404
    assert request(f"{served}/map.bin", "PUT", b"\x08\x01")[0] == 204
    assert map_file.read_bytes() == b"\x08\x01"
    assert request(f"{served}/map.bin?v=1") == (200, b"\x08\x01")
    assert request(f"{served}/other.bin", "PUT", b"x")[0] == 404
    assert not (map_file.parent / "other.bin").exists()


def test_without_a_file_there_is_nothing_to_read_or_write(static: Path, tmp_path: Path) -> None:
    server = editor.make_server(None)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    url = f"http://127.0.0.1:{server.server_address[1]}"
    try:
        assert request(f"{url}/")[0] == 200
        assert request(f"{url}/map.bin")[0] == 404
        assert request(f"{url}/map.bin", "PUT", b"\x08\x01")[0] == 404
        assert not list(static.glob("*.bin"))
    finally:
        server.shutdown()
        server.server_close()


def test_a_saved_map_plays(served: str, map_file: Path) -> None:
    request(f"{served}/map.bin", "PUT", STANDARD.read_bytes())
    noop = str(ROOT / "bots" / "ucbc2027" / "noop")
    args = ["run", noop, noop, "--game", "ucbc2027", "--sets", "1", "--map", str(map_file)]
    result = CliRunner().invoke(main, args)
    assert result.exit_code == 0, result.output


def test_refuses_an_unbuilt_editor(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(editor, "STATIC", tmp_path)
    with pytest.raises(click.ClickException, match="make editor"):
        editor.make_server(tmp_path / "arena.map")


def test_editor_opens_the_file(map_file: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    opened: list[tuple[Path | None, int, bool]] = []
    monkeypatch.setattr(
        editor, "open_editor", lambda *args, **kw: opened.append((*args, kw["browser"]))
    )
    result = CliRunner().invoke(main, ["editor", str(map_file), "--port", "8123", "--no-browser"])
    assert result.exit_code == 0, result.output
    assert opened == [(map_file, 8123, False)]
    result = CliRunner().invoke(main, ["editor", "--no-browser"])
    assert result.exit_code == 0, result.output
    assert opened[1] == (None, 0, False)
