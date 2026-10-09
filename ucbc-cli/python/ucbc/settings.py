import os
from urllib.parse import urlparse

import click

LOCALHOST = ["127.0.0.1", "localhost"]


def https_check(url: str) -> None:
    """
    Checks if URL is HTTPS or localhost, raises error if it is not on HTTPS.
    localhost connections are allowed, so local dev still works.
    """
    u = urlparse(url)
    if not ((u.scheme == "https") or (u.scheme == "http" and u.hostname in LOCALHOST)):
        raise click.ClickException(
            f"UCBC_SERVER must use https (http only for localhost): got {url}"
        )


def server_url() -> str:
    """Returns the server url set (UCBC_SERVER) or default (production)"""
    url = os.environ.get("UCBC_SERVER", "https://ucbc.andrewheschl.ca/api")
    https_check(url)
    return url
