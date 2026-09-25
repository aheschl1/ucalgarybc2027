from datetime import datetime

from pydantic import BaseModel


class Team(BaseModel):
    """A `teams` row: a participant team, which owns submissions."""

    id: int
    name: str
    created_at: datetime
