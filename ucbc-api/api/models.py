from datetime import datetime
from typing import Any, Literal
from uuid import UUID

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


# The match types mirror the engine's replay JSON (ucbc-engine/src/replay.rs).


class TeamInfo(BaseModel):
    id: int
    name: str


class SetResult(BaseModel):
    index: int
    first_team: int
    winner_team: int | None
    reason: Literal["win", "draw", "forfeit"]
    detail: str = ""
    ticks: int


class SetReplay(BaseModel):
    index: int
    first_team: int
    initial_state: Any
    ticks: list[Any]
    result: SetResult


class MatchResult(BaseModel):
    sets: list[SetResult]
    set_wins: list[int]
    winner_team: int | None


class MatchCreate(BaseModel):
    game: str
    engine_version: str
    teams: list[TeamInfo]
    config: dict[str, Any]


class MatchCreated(BaseModel):
    id: UUID


class Match(BaseModel):
    id: UUID
    game: str
    engine_version: str
    teams: list[TeamInfo]
    config: dict[str, Any]
    status: Literal["running", "done"]
    sets: list[SetResult]
    set_wins: list[int] | None
    winner_team: int | None
    created_at: datetime
    completed_at: datetime | None
