from fastapi import APIRouter, status

from api.auth import AdminUser, CurrentUser
from api.db import DB
from api.models.users import User, UserCreate
from api.services.users import create_user

router = APIRouter(prefix="/users", tags=["users"])


@router.post("", status_code=status.HTTP_201_CREATED)
async def create(body: UserCreate, db: DB, _user: AdminUser) -> User:
    return await create_user(db, body.username, body.password, body.is_admin)


@router.get("/me")
async def me(user: CurrentUser) -> User:
    return user
