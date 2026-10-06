from fastapi import APIRouter, status

from api.auth import CurrentUser
from api.db import DB
from api.models.users import User, UserCreate
from api.services.users import create_user

router = APIRouter(prefix="/users", tags=["users"])


@router.post("", status_code=status.HTTP_201_CREATED)
async def create(body: UserCreate, db: DB) -> User:
    """Sign up. Admins are made under /admin/users, never here."""
    return await create_user(db, body.email, body.display_name, body.password, is_admin=False)


@router.get("/me")
async def me(user: CurrentUser) -> User:
    return user
