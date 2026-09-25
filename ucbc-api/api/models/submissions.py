from datetime import datetime
from uuid import UUID

from pydantic import BaseModel


class Submission(BaseModel):
    """A `submissions` row with its uploader's display name and its team's name, which is
    how the repo always reads one."""

    id: UUID
    user_id: int
    team_id: int
    name: str
    game: str
    size: int
    sha256: str
    created_at: datetime
    display_name: str
    team_name: str
