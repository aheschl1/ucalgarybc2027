from datetime import datetime
from uuid import UUID

from pydantic import BaseModel


class SubmissionRow(BaseModel):
    """A `submissions` row."""

    id: UUID
    user_id: int
    name: str
    game: str
    size: int
    sha256: str
    created_at: datetime


class Submission(SubmissionRow):
    """A submission with its owner's name."""

    username: str
