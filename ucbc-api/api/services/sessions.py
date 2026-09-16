"""Browser sessions: an opaque token in a cookie, its hash in the `sessions` table."""

import hashlib
import secrets
from datetime import UTC, datetime, timedelta

from api.db import DBConnection
from api.models.users import User
from api.services.users import authenticate

COOKIE = "ucbc_session"
LIFETIME = timedelta(days=30)


def hash_token(token: str) -> str:
    return hashlib.sha256(token.encode()).hexdigest()


async def log_in(db: DBConnection, username: str, password: str) -> tuple[str, User] | None:
    """A fresh token for the user, or None when the credentials are wrong."""
    user = await authenticate(db, username, password)
    if user is None:
        return None
    await db.session_repo.delete_expired(user.id)
    token = secrets.token_urlsafe(32)
    await db.session_repo.insert(hash_token(token), user.id, datetime.now(UTC) + LIFETIME)
    return token, user


async def resolve(db: DBConnection, token: str) -> User | None:
    stored = await db.session_repo.get_user(hash_token(token))
    return None if stored is None else stored.public()


async def log_out(db: DBConnection, token: str) -> None:
    await db.session_repo.delete(hash_token(token))
