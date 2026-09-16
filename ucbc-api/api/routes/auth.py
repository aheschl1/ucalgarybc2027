from typing import Annotated

from fastapi import APIRouter, Cookie, HTTPException, Request, Response, status
from slowapi import Limiter
from slowapi.util import get_remote_address

from api.db import DB
from api.models.sessions import Credentials
from api.models.users import User
from api.services import sessions
from api.services.sessions import COOKIE, LIFETIME

router = APIRouter(prefix="/auth", tags=["auth"])
# Password guessing is throttled per client address; the app registers the 429 handler.
limiter = Limiter(key_func=get_remote_address)
LOGIN_LIMIT = "60/minute"


def set_session_cookie(response: Response, token: str | None) -> None:
    """Set the cookie, or clear it when `token` is None. SameSite=Strict keeps it off
    cross-site requests, which is the CSRF defence. Always Secure: browsers treat
    localhost as a secure context, so local dev over http still works."""
    response.set_cookie(
        COOKIE,
        token or "",
        max_age=int(LIFETIME.total_seconds()) if token else 0,
        path="/",
        secure=True,
        httponly=True,
        samesite="strict",
    )


@router.post("/login")
@limiter.limit(LOGIN_LIMIT)
async def login(request: Request, body: Credentials, db: DB, response: Response) -> User:
    result = await sessions.log_in(db, body.email, body.password)
    if result is None:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "wrong email or password")
    token, user = result
    set_session_cookie(response, token)
    return user


@router.post("/logout", status_code=status.HTTP_204_NO_CONTENT)
async def logout(
    db: DB, response: Response, token: Annotated[str | None, Cookie(alias=COOKIE)] = None
) -> None:
    if token is not None:
        await sessions.log_out(db, token)
    set_session_cookie(response, None)
