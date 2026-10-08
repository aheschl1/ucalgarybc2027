from pathlib import PurePath
from typing import Annotated
from uuid import UUID

from fastapi import APIRouter, Form, Response, UploadFile, status

from api.auth import AdminUser, CurrentUser
from api.blobs import Blobs
from api.db import DB
from api.errors import PayloadTooLarge
from api.models.maps import Map, MapUpdate
from api.services import maps
from api.services.maps import MAX_MAP

router = APIRouter(prefix="/maps", tags=["maps"])


@router.post("", status_code=status.HTTP_201_CREATED)
async def upload(
    file: UploadFile,
    game: Annotated[str, Form()],
    db: DB,
    blobs: Blobs,
    user: AdminUser,
    name: Annotated[str | None, Form()] = None,
) -> Map:
    """Upload a map file for a game. The name defaults to the file's."""
    data = await file.read(MAX_MAP + 1)
    if len(data) > MAX_MAP:
        raise PayloadTooLarge(f"map is larger than {MAX_MAP >> 10} KiB")
    name = name or PurePath(file.filename or "map").stem
    return await maps.upload(db, blobs, user, name, game, data)


@router.get("")
async def list_maps(db: DB, _user: CurrentUser, game: str | None = None) -> list[Map]:
    """Every map, archived ones included, by game then name."""
    return await maps.list_maps(db, game)


@router.get("/{map_id}")
async def get(map_id: UUID, db: DB, _user: CurrentUser) -> Map:
    return await maps.get_map(db, map_id)


@router.get("/{map_id}/file", response_class=Response)
async def get_file(map_id: UUID, db: DB, blobs: Blobs, _user: AdminUser) -> Response:
    data = await maps.read_file(db, blobs, map_id)
    return Response(data, media_type="application/octet-stream")


@router.patch("/{map_id}")
async def update(map_id: UUID, body: MapUpdate, db: DB, _user: AdminUser) -> Map:
    """Archive a map, so no new match picks it, or bring it back."""
    return await maps.set_archived(db, map_id, body.archived)
