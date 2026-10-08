from pathlib import Path

import click
import pytest
from ucbc.cli import session
from ucbc.settings import server_url

from collections.abc import Callable
from click.testing import CliRunner
from ucbc.cli import main

BotPath = Callable[[str], Path]

def test_server_url_accepts_https_localhost(monkeypatch: pytest.MonkeyPatch) -> None:
    urls = ["https://ucbc.andrewheschl.ca/api", "http://localhost:9", "http://127.0.0.1:9"]
    for url in urls:
        monkeypatch.setenv("UCBC_SERVER", url)
        assert server_url() == url


def test_server_url_refuses_not_https_localhost(monkeypatch: pytest.MonkeyPatch) -> None:
    urls = ["http://example.com/api", "htps://example.com/api", "ftp://example"]
    for url in urls:
        monkeypatch.setenv("UCBC_SERVER", url)
        with pytest.raises(click.ClickException, match="must use https"):
            server_url()


def test_session_lifecycle(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(session, "SESSION_FILE", tmp_path / "session.json")
    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    session.save("abc")
    assert session.load() == "abc"

    monkeypatch.setenv("UCBC_SERVER", "http://127.0.0.1:8000/api")
    assert session.load() is None

    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    session.clear()
    assert not session.SESSION_FILE.exists()    
    assert session.load() is None

def test_not_logged_in_submit(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, bot: BotPath) -> None:
    monkeypatch.setattr(session, "SESSION_FILE", tmp_path / "session.json")
    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    args = ["submit", str(bot("first_empty")), "--game", "tictactoe"]
    result = CliRunner().invoke(main, args)    
    assert result.exit_code != 0, result.output
    assert "not logged in" in result.output    