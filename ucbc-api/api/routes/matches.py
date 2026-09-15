from typing import Any
from uuid import UUID

from fastapi import APIRouter, status

from api.auth import AdminUser
from api.db import DB
from api.models import Match, MatchCreate, MatchCreated, MatchResult, SetReplay
from api.services import matches

# Admin only until matches are linked to users and visibility rules exist.
router = APIRouter(prefix="/matches", tags=["matches"])


@router.post("", status_code=status.HTTP_201_CREATED)
async def create(body: MatchCreate, db: DB, _user: AdminUser) -> MatchCreated:
    return MatchCreated(id=await matches.create_match(db, body))


@router.post("/{match_id}/sets", status_code=status.HTTP_204_NO_CONTENT)
async def add_set(match_id: UUID, body: SetReplay, db: DB, _user: AdminUser) -> None:
    await matches.add_set(db, match_id, body)


@router.post("/{match_id}/complete", status_code=status.HTTP_204_NO_CONTENT)
async def complete(match_id: UUID, body: MatchResult, db: DB, _user: AdminUser) -> None:
    await matches.complete_match(db, match_id, body)


@router.get("/{match_id}")
async def get(match_id: UUID, db: DB, _user: AdminUser) -> Match:
    return await matches.get_match(db, match_id)


@router.get("/{match_id}/sets/{index}")
async def get_set(match_id: UUID, index: int, db: DB, _user: AdminUser) -> dict[str, Any]:
    return await matches.get_set_replay(db, match_id, index)
