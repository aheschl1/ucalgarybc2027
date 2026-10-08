from psycopg.errors import ForeignKeyViolation, UniqueViolation

from api.db import DBConnection
from api.errors import ApiError, Conflict, NotFound
from api.models.teams import Team
from api.models.users import User
from api.passwords import hash_password, verify_password
from api.services.teams import new_code


def normalise(email: str) -> str:
    """One spelling per address, so the unique constraint is the whole story."""
    return email.strip().lower()


async def create_user(
    db: DBConnection, email: str, display_name: str, password: str, is_admin: bool
) -> User:
    """The user and a team of their own."""
    try:
        async with db.conn.transaction():
            team = await _solo_team(db, display_name)
            stored = await db.user_repo.insert(
                normalise(email), display_name, hash_password(password), is_admin, team.id
            )
    except UniqueViolation as e:
        raise Conflict(f"{email} already has an account") from e
    return stored.public()


async def delete_user(db: DBConnection, actor: User, user_id: int) -> None:
    """Their sessions go with them; their team and the maps they uploaded stay."""
    if user_id == actor.id:
        raise ApiError("you cannot delete yourself")
    try:
        async with db.conn.transaction():
            deleted = await db.user_repo.delete(user_id)
    except ForeignKeyViolation as e:
        raise Conflict(f"user {user_id} uploaded submissions") from e
    if not deleted:
        raise NotFound(f"no user {user_id}")


async def _solo_team(db: DBConnection, name: str) -> Team:
    """A new team called `name`, or `name 2`, `name 3`, ... when that is taken. Each try is
    a savepoint, so a clash leaves the caller's transaction usable."""
    n = 1
    while True:
        try:
            async with db.conn.transaction():
                return await db.team_repo.insert(name if n == 1 else f"{name} {n}", new_code())
        except UniqueViolation:
            n += 1


async def authenticate(db: DBConnection, email: str, password: str) -> User | None:
    stored = await db.user_repo.get_by_email(normalise(email))
    if stored is None or not await verify_password(stored.password_hash, password):
        return None
    return stored.public()
