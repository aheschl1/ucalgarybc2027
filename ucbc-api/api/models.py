from datetime import datetime

from pydantic import BaseModel, Field


class User(BaseModel):
    id: int
    username: str
    is_admin: bool
    created_at: datetime


class StoredUser(User):
    password_hash: str

    def public(self) -> User:
        return User(
            id=self.id, username=self.username, is_admin=self.is_admin, created_at=self.created_at
        )


class UserCreate(BaseModel):
    username: str = Field(min_length=1, max_length=64)
    password: str = Field(min_length=1)
    is_admin: bool = False
