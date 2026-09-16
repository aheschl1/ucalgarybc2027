from psycopg.errors import UniqueViolation

from api.db import DBConnection
from api.errors import Conflict
from api.models.users import User
from api.passwords import hash_password, verify_password


def normalise(email: str) -> str:
    """One spelling per address, so the unique constraint is the whole story."""
    return email.strip().lower()


async def create_user(
    db: DBConnection, email: str, display_name: str, password: str, is_admin: bool
) -> User:
    try:
        stored = await db.user_repo.insert(
            normalise(email), display_name, hash_password(password), is_admin
        )
    except UniqueViolation as e:
        raise Conflict(f"{email} already has an account") from e
    return stored.public()


async def authenticate(db: DBConnection, email: str, password: str) -> User | None:
    stored = await db.user_repo.get_by_email(normalise(email))
    if stored is None or not await verify_password(stored.password_hash, password):
        return None
    return stored.public()
