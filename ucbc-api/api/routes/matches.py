from uuid import UUID

from fastapi import APIRouter, status

from api.auth import CurrentUser
from api.db import DB
from api.models.matches import Match, MatchCreated, MatchEnqueue, MatchRow, SetReplay
from api.services import matches

router = APIRouter(prefix="/matches", tags=["matches"])


@router.post("/queue", status_code=status.HTTP_201_CREATED)
async def enqueue(body: MatchEnqueue, db: DB, user: CurrentUser) -> MatchCreated:
    """Queue a match for a worker to play. Members use submissions, one of them their own."""
    return MatchCreated(id=await matches.enqueue_match(db, user, body))


@router.get("")
async def list_matches(db: DB, user: CurrentUser) -> list[MatchRow]:
    """Platform matches and the caller's own, newest first; every match for an admin."""
    return await matches.list_matches(db, user)


@router.get("/{match_id}")
async def get(match_id: UUID, db: DB, user: CurrentUser) -> Match:
    return await matches.get_match(db, user, match_id)


@router.get("/{match_id}/sets/{index}", response_model_exclude_unset=True)
async def get_set(match_id: UUID, index: int, db: DB, user: CurrentUser) -> SetReplay:
    return await matches.get_set_replay(db, user, match_id, index)
