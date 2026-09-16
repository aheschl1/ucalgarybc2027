from datetime import datetime

from pydantic import BaseModel, EmailStr, Field


class User(BaseModel):
    """The caller's own user. It carries an email, so it is should not be returned for anyone
    else."""

    id: int
    email: str
    display_name: str
    is_admin: bool
    created_at: datetime


class StoredUser(User):
    """A `users` row."""

    password_hash: str

    def public(self) -> User:
        # `User` has no password_hash, so pydantic drops it.
        return User.model_validate(self.model_dump())


class UserCreate(BaseModel):
    """Self-serve signup. Admins are made with `ucbc-api-cli create-admin`."""

    email: EmailStr
    display_name: str = Field(min_length=1, max_length=64)
    password: str = Field(min_length=8, max_length=128)
