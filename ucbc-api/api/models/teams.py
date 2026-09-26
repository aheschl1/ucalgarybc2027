from datetime import datetime
from typing import Annotated

from pydantic import BaseModel, StringConstraints

TeamName = Annotated[str, StringConstraints(strip_whitespace=True, min_length=1, max_length=64)]


class Team(BaseModel):
    """A `teams` row: a participant team, which owns submissions. It carries the join code,
    so only a member is shown one."""

    id: int
    name: str
    join_code: str
    created_at: datetime


class MyTeam(Team):
    """The caller's team with its members' display names."""

    members: list[str]


class TeamCreate(BaseModel):
    name: TeamName


class TeamJoin(BaseModel):
    code: str
