import click
import pytest
from ucbc.settings import server_url

def test_server_url_accepts_https_localhost(monkeypatch: pytest.MonkeyPatch) -> None:
    urls = ["https://ucbc.andrewheschl.ca/api", "http://localhost:9", "http://127.0.0.1:9"]
    for url in urls:
        monkeypatch.setenv("UCBC_SERVER", url)    
        assert server_url() in url


def test_server_url_refuses_not_https_localhost(monkeypatch: pytest.MonkeyPatch) -> None:
    urls = ["http://example.com/api", "htps://example.com/api", "ftp://example"]
    for url in urls:
        monkeypatch.setenv("UCBC_SERVER", url)
        with pytest.raises(click.ClickException, match= "must use https"):
            server_url()
