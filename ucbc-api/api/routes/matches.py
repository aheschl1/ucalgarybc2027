from uuid import UUID

from fastapi import APIRouter, Response, status

from api.auth import CurrentUser
from api.blobs import Blobs
from api.db import DB
from api.models.matches import Match, MatchCreated, MatchEnqueue, MatchRow
from api.services import matches

router = APIRouter(prefix="/matches", tags=["matches"])


@router.post("/queue", status_code=status.HTTP_201_CREATED)
async def enqueue(body: MatchEnqueue, db: DB, user: CurrentUser) -> MatchCreated:
    """Queue a match for a worker to play. Members use submissions, one their team's."""
    return MatchCreated(id=await matches.enqueue_match(db, user, body))


@router.get("")
async def list_matches(db: DB, user: CurrentUser, mine: bool = False) -> list[MatchRow]:
    """Platform matches and the caller's team's, newest first; every match for an admin.
    `mine` narrows to matches the caller's team has a bot in."""
    return await matches.list_matches(db, user, mine)


@router.get("/{match_id}")
async def get(match_id: UUID, db: DB, user: CurrentUser) -> Match:
    return await matches.get_match(db, user, match_id)


@router.get("/{match_id}/sets/{index}", response_class=Response)
async def get_set(match_id: UUID, index: int, db: DB, user: CurrentUser, blobs: Blobs) -> Response:
    """One set of the replay as JSON, sent gzipped: the browser decompresses it, so the
    page reads it like any other JSON."""
    data = await matches.get_set_replay(db, user, match_id, index, blobs)
    return Response(data, media_type="application/json", headers={"Content-Encoding": "gzip"})
