from psycopg.errors import UniqueViolation

from api.db import DBConnection
from api.errors import Conflict
from api.models.users import User
from api.passwords import hash_password, verify_password


async def create_user(db: DBConnection, username: str, password: str, is_admin: bool) -> User:
    try:
        stored = await db.user_repo.insert(username, hash_password(password), is_admin)
    except UniqueViolation as e:
        raise Conflict(f"username {username!r} is taken") from e
    return stored.public()


async def authenticate(db: DBConnection, username: str, password: str) -> User | None:
    stored = await db.user_repo.get_by_username(username)
    if stored is None or not await verify_password(stored.password_hash, password):
        return None
    return stored.public()
