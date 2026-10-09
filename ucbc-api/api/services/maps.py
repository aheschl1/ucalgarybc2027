"""Map files, uploaded by admins and picked per set when a match is queued. The file is in
blob storage under a key derived from the map id. The API does not read map files: one the
game refuses fails the match it is played in, with the game's reason as the error."""

import hashlib
from uuid import UUID, uuid4

from psycopg.errors import UniqueViolation

from api.blobs import BlobStore
from api.db import DBConnection
from api.errors import ApiError, Conflict, NotFound
from api.models.maps import Map
from api.models.users import User

MAX_MAP = 64 << 10
MAX_NAME = 64


def key_for(id: UUID) -> str:
    return f"maps/{id}.map"


async def upload(
    db: DBConnection, blobs: BlobStore, user: User | None, name: str, game: str, data: bytes
) -> Map:
    """`user` is None from the CLI."""
    if not 1 <= len(name) <= MAX_NAME:
        raise ApiError(f"name must be 1 to {MAX_NAME} characters")
    id = uuid4()
    await blobs.put(key_for(id), data, "application/octet-stream")
    try:
        # A savepoint, so a clash leaves the caller's transaction usable.
        async with db.conn.transaction():
            await db.map_repo.insert(
                id,
                game,
                name,
                len(data),
                hashlib.sha256(data).hexdigest(),
                None if user is None else user.id,
            )
    except UniqueViolation as e:
        await blobs.delete(key_for(id))
        raise Conflict(f"{game} already has a map named {name!r}") from e
    except Exception:
        await blobs.delete(key_for(id))
        raise
    return await get_map(db, id)


async def get_map(db: DBConnection, id: UUID) -> Map:
    found = await db.map_repo.get(id)
    if found is None:
        raise NotFound(f"no map {id}")
    return found


async def list_maps(db: DBConnection, game: str | None) -> list[Map]:
    return await db.map_repo.list_maps(game)


async def set_archived(db: DBConnection, id: UUID, archived: bool) -> Map:
    if not await db.map_repo.set_archived(id, archived):
        raise NotFound(f"no map {id}")
    return await get_map(db, id)


async def read_file(db: DBConnection, blobs: BlobStore, id: UUID) -> bytes:
    await get_map(db, id)
    return await blobs.get(key_for(id))
