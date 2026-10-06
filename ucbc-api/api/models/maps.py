from datetime import datetime
from uuid import UUID

from pydantic import BaseModel


class Map(BaseModel):
    """A `maps` row. The file is in the bucket; an archived map stays readable, so the
    matches played on it still name it, but is never picked for a new one."""

    id: UUID
    game: str
    name: str
    size: int
    sha256: str
    uploaded_by: int | None
    created_at: datetime
    archived_at: datetime | None


class MapUpdate(BaseModel):
    archived: bool
