import gzip
import threading
import urllib.request
from collections.abc import Callable, Iterator
from pathlib import Path

import click
import pytest
from click.testing import CliRunner
from ucbc import viewer
from ucbc.cli import main

BotPath = Callable[[str], Path]


@pytest.fixture
def static(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A stand-in for the built page, so these tests do not need Node."""
    page = tmp_path / "static"
    (page / "assets").mkdir(parents=True)
    (page / "index.html").write_text("<title>viewer</title>")
    (page / "assets" / "app.js").write_text("app")
    monkeypatch.setattr(viewer, "STATIC", page)
    return page


@pytest.fixture
def served(static: Path, tmp_path: Path) -> Iterator[str]:
    replay = tmp_path / "r.json"
    replay.write_text('{"sets": []}')
    server = viewer.make_server(replay)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{server.server_address[1]}"
    server.shutdown()
    server.server_close()


def get(url: str) -> tuple[str, str]:
    with urllib.request.urlopen(url) as r:
        return r.read().decode(), r.headers["Cache-Control"]


def test_serves_the_page_and_the_replay(served: str) -> None:
    assert get(f"{served}/")[0] == "<title>viewer</title>"
    assert get(f"{served}/assets/app.js")[0] == "app"
    assert get(f"{served}/replay.json") == ('{"sets": []}', "no-cache")
    assert get(f"{served}/replay.json?v=1")[0] == '{"sets": []}'


def test_serves_a_gzipped_replay_for_the_browser_to_decompress(
    static: Path, tmp_path: Path
) -> None:
    replay = tmp_path / "r.json.gz"
    replay.write_bytes(gzip.compress(b'{"sets": []}'))
    server = viewer.make_server(replay)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        url = f"http://127.0.0.1:{server.server_address[1]}/replay.json"
        with urllib.request.urlopen(url) as r:
            assert r.headers["Content-Encoding"] == "gzip"
            assert r.headers["Content-Type"] == "application/json"
            assert gzip.decompress(r.read()) == b'{"sets": []}'
    finally:
        server.shutdown()
        server.server_close()


def test_refuses_an_unbuilt_viewer(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(viewer, "STATIC", tmp_path)
    with pytest.raises(click.ClickException, match="make viewer"):
        viewer.make_server(tmp_path / "r.json")


def test_run_view_writes_a_replay_and_opens_it(
    bot: BotPath, static: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    opened: list[Path] = []
    monkeypatch.setattr(viewer, "open_viewer", lambda replay: opened.append(replay))
    result = CliRunner().invoke(
        main,
        ["run", str(bot("first_empty")), str(bot("random")), "--game", "tictactoe", "--view"],
    )
    assert result.exit_code == 0, result.output
    assert len(opened) == 1 and opened[0].is_file()


def test_run_view_fails_before_the_match_when_unbuilt(
    bot: BotPath, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(viewer, "STATIC", tmp_path)
    result = CliRunner().invoke(
        main,
        ["run", str(bot("first_empty")), str(bot("random")), "--game", "tictactoe", "--view"],
    )
    assert result.exit_code != 0
    assert "make viewer" in result.output
    assert "Set 1:" not in result.output
