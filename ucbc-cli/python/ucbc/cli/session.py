"""Used for logging in on CLI. Stores the session token in a json file. Checks if the session url matches the server url"""

import json
import os
from pathlib import Path

from ucbc.settings import server_url

COOKIE = "__Host-ucbc_session"
SESSION_FILE = Path.home() / ".config" / "ucbc" / "session.json"


def save(token: str) -> None:
    """Remember the session token for the current server"""
    SESSION_FILE.parent.mkdir(parents=True, exist_ok=True)
    SESSION_FILE.touch(mode=0o600)  # sets permission to only file owner has read and write access
    os.chmod(SESSION_FILE, 0o600)  # in case json file already existed
    SESSION_FILE.write_text(json.dumps({"server": server_url(), "token": token}))


def load() -> str | None:
    """Load the session token, returns None if there is no token or belongs to a different server"""
    if not SESSION_FILE.exists():
        return None
    body = json.loads(SESSION_FILE.read_text())
    if body["server"] != server_url():
        return None
    return str(body["token"])


def clear() -> None:
    """Deletes the saved session file"""
    SESSION_FILE.unlink(missing_ok=True)


def auth_headers() -> dict[str, str]:
    """Formats the token into a cookie header, returns empty dict {} if there's no saved token"""
    token = load()
    if token is None:
        return {}
    return {"Cookie": f"{COOKIE}={token}"}
