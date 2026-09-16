"""Request dependencies that resolve a user from the session cookie."""

from typing import Annotated

from fastapi import Cookie, Depends, HTTPException, status

from api.db import DB
from api.models.users import User
from api.services import sessions
from api.services.sessions import COOKIE


async def current_user(db: DB, token: Annotated[str | None, Cookie(alias=COOKIE)] = None) -> User:
    if token is None:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "log in")
    user = await sessions.resolve(db, token)
    if user is None:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "session expired; log in again")
    return user


async def admin_user(user: Annotated[User, Depends(current_user)]) -> User:
    if not user.is_admin:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "admin only")
    return user


CurrentUser = Annotated[User, Depends(current_user)]
AdminUser = Annotated[User, Depends(admin_user)]
