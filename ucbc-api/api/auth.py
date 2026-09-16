"""Request dependencies that resolve a user from HTTP Basic headers."""

from typing import Annotated

from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPBasic, HTTPBasicCredentials

from api.db import DB
from api.models.users import User
from api.services.users import authenticate


async def current_user(
    creds: Annotated[HTTPBasicCredentials, Depends(HTTPBasic())], db: DB
) -> User:
    user = await authenticate(db, creds.username, creds.password)
    if user is None:
        raise HTTPException(
            status.HTTP_401_UNAUTHORIZED,
            "wrong username or password",
            headers={"WWW-Authenticate": "Basic"},
        )
    return user


async def admin_user(user: Annotated[User, Depends(current_user)]) -> User:
    if not user.is_admin:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "admin only")
    return user


CurrentUser = Annotated[User, Depends(current_user)]
AdminUser = Annotated[User, Depends(admin_user)]
