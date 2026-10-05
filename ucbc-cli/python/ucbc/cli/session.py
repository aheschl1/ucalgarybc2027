import json
import os
from pathlib import Path

from ucbc.settings import server_url

COOKIE = "__Host-ucbc_session"
SESSION_FILE = Path.home() / ".config" / "ucbc" / "session.json"

def save(token: str) -> None:
    """Remember the session token for the current server"""
    SESSION_FILE.parent.mkdir(parents=True, exist_ok=True)
    SESSION_FILE.touch(mode=0o600) # sets permission to only file owner has read and write access
    os.chmod(SESSION_FILE, 0o600) # in case json file already existed
    SESSION_FILE.write_text(json.dumps({"server": server_url(), "token": token}))

def load() -> str | None:
    """Load the session token"""
    if not SESSION_FILE.exists():
        return None
    body = json.loads(SESSION_FILE.read_text())
    if body['server'] != server_url():
        return None
    return body['token']

def clear() -> None:
    pass

def auth_headers() -> dict[str, str]:
    pass