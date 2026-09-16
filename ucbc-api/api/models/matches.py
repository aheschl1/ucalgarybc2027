from datetime import datetime
from pathlib import PurePath
from typing import Annotated, Any, Literal
from uuid import UUID

from pydantic import BaseModel, Field

# The replay types mirror the engine's replay JSON (ucbc-engine/src/replay.rs).


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


class MatchReplay(BaseModel):
    """The file `ucbc run --replay` writes."""

    match_id: str
    engine_version: str
    config: dict[str, Any]
    teams: list[TeamInfo]
    sets: list[SetReplay]
    result: MatchResult


class MatchConfig(BaseModel):
    """What `ucbc run` is told; the defaults are its own. The API does not import the engine."""

    sets: int = Field(default=3, ge=1)
    seed: int = 0
    step_ms: int = Field(default=500, ge=1)
    memory_bytes: int = Field(default=2**30, ge=1)


class PathSource(BaseModel):
    """A bot directory on the worker host."""

    kind: Literal["path"] = "path"
    path: str

    @property
    def name(self) -> str:
        return PurePath(self.path).name


class SubmissionSource(BaseModel):
    """An uploaded submission, fetched from blob storage."""

    kind: Literal["submission"] = "submission"
    id: UUID


BotSource = Annotated[PathSource | SubmissionSource, Field(discriminator="kind")]

MatchStatus = Literal["queued", "running", "done", "error"]
# Who made the match: a user, or the platform from a schedule. Platform matches are public.
MatchOrigin = Literal["user", "platform"]


class MatchEnqueue(BaseModel):
    """A match for a worker to play."""

    game: str
    bots: list[BotSource] = Field(min_length=2, max_length=2)
    config: MatchConfig = MatchConfig()
    priority: int = 0


class MatchCreated(BaseModel):
    id: UUID


class MatchRow(BaseModel):
    """A `matches` row."""

    id: UUID
    origin: MatchOrigin
    game: str
    engine_version: str | None
    teams: list[TeamInfo]
    config: MatchConfig
    bots: list[BotSource]
    status: MatchStatus
    set_wins: list[int] | None
    winner_team: int | None
    error: str | None
    priority: int
    attempts: int
    max_attempts: int
    claimed_by: str | None
    claimed_at: datetime | None
    heartbeat_at: datetime | None
    created_at: datetime
    completed_at: datetime | None


class Match(MatchRow):
    """A match with its set results."""

    sets: list[SetResult]
