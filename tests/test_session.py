import uuid
from collections.abc import Callable
from pathlib import Path

import click
import httpx
import pytest
from click.testing import CliRunner
from ucbc.cli import main, session
from ucbc.settings import server_url

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


def test_not_logged_in_submit(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, bot: BotPath
) -> None:
    monkeypatch.setattr(session, "SESSION_FILE", tmp_path / "session.json")
    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    args = ["submit", str(bot("first_empty")), "--game", "tictactoe"]
    result = CliRunner().invoke(main, args)
    assert result.exit_code != 0, result.output
    assert "not logged in" in result.output


def test_not_logged_in_queue(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(session, "SESSION_FILE", tmp_path / "session.json")
    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    uuid1 = str(uuid.uuid4())
    uuid2 = str(uuid.uuid4())
    args = ["queue", uuid1, uuid2]
    result = CliRunner().invoke(main, args)
    assert result.exit_code == 1, result.output
    assert "not logged in" in result.output


def test_not_logged_in_submissions(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(session, "SESSION_FILE", tmp_path / "session.json")
    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    args = ["submissions"]
    result = CliRunner().invoke(main, args)
    assert result.exit_code == 1, result.output
    assert "not logged in" in result.output


def test_check_status_detail() -> None:
    response = httpx.Response(404, json={"detail": "error not found"})
    with pytest.raises(click.ClickException, match="error not found"):
        session.check_status(response)


def test_check_status_success() -> None:
    response = httpx.Response(200)
    session.check_status(response)


def test_check_status_401_clears_session(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(session, "SESSION_FILE", tmp_path / "session.json")
    monkeypatch.setenv("UCBC_SERVER", "http://localhost:8000/api")
    session.save("abc")
    response = httpx.Response(401)

    with pytest.raises(click.ClickException, match="session expired"):
        session.check_status(response)

    assert not session.SESSION_FILE.exists()
