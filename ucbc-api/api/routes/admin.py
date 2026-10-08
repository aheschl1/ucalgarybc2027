from fastapi import APIRouter, status

from api.auth import AdminUser
from api.db import DB
from api.models.users import AdminUserCreate, User
from api.services.users import create_user, delete_user

router = APIRouter(prefix="/admin", tags=["admin"])


@router.get("/users")
async def list_users(db: DB, _user: AdminUser) -> list[User]:
    return [u.public() for u in await db.user_repo.list()]


@router.post("/users", status_code=status.HTTP_201_CREATED)
async def create(body: AdminUserCreate, db: DB, _user: AdminUser) -> User:
    return await create_user(db, body.email, body.display_name, body.password, body.is_admin)


@router.delete("/users/{user_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete(user_id: int, db: DB, user: AdminUser) -> None:
    await delete_user(db, user, user_id)
