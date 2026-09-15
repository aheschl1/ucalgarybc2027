from fastapi import APIRouter, HTTPException, status

from api.auth import AdminUser, CurrentUser
from api.db import DB
from api.models import User, UserCreate
from api.services.users import UsernameTaken, create_user

router = APIRouter(prefix="/users", tags=["users"])


@router.post("", status_code=status.HTTP_201_CREATED)
async def create(body: UserCreate, db: DB, _user: AdminUser) -> User:
    try:
        return await create_user(db, body.username, body.password, body.is_admin)
    except UsernameTaken as e:
        raise HTTPException(status.HTTP_409_CONFLICT, str(e)) from e


@router.get("/me")
async def me(user: CurrentUser) -> User:
    return user
