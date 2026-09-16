from uuid import UUID

from fastapi import APIRouter, status

from api.auth import AdminUser
from api.db import DB
from api.models.matches import Match, MatchCreated, MatchEnqueue, SetReplay
from api.services import matches

# Admin only until matches are linked to users and visibility rules exist.
router = APIRouter(prefix="/matches", tags=["matches"])


@router.post("/queue", status_code=status.HTTP_201_CREATED)
async def enqueue(body: MatchEnqueue, db: DB, _user: AdminUser) -> MatchCreated:
    """Queue a match for a worker to play."""
    return MatchCreated(id=await matches.enqueue_match(db, body))


@router.get("/{match_id}")
async def get(match_id: UUID, db: DB, _user: AdminUser) -> Match:
    return await matches.get_match(db, match_id)


@router.get("/{match_id}/sets/{index}", response_model_exclude_unset=True)
async def get_set(match_id: UUID, index: int, db: DB, _user: AdminUser) -> SetReplay:
    return await matches.get_set_replay(db, match_id, index)
