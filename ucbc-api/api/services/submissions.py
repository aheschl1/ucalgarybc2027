"""Uploaded bot code: a zip with main.py at the top, kept in blob storage under a key
derived from the submission id."""

import hashlib
import io
import zipfile
from pathlib import PurePosixPath
from uuid import UUID, uuid4

from api.blobs import BlobStore
from api.db import DBConnection
from api.errors import ApiError, NotFound
from api.models.submissions import Submission
from api.models.users import User

MAX_ZIP = 1 << 20
MAX_UNPACKED = 8 << 20
MAX_NAME = 64


def key_for(id: UUID) -> str:
    return f"submissions/{id}.zip"


def check_member_name(name: str) -> None:
    """Rejects a zip member that could land outside the bot directory."""
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts or "\\" in name or name.startswith("/"):
        raise ApiError(f"bad path in zip: {name!r}")


def check_zip(data: bytes) -> None:
    try:
        zf = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile as e:
        raise ApiError("not a zip file") from e
    if zf.testzip() is not None:
        raise ApiError("corrupt zip")
    if sum(i.file_size for i in zf.infolist()) > MAX_UNPACKED:
        raise ApiError(f"zip unpacks to more than {MAX_UNPACKED >> 20} MiB")
    names = zf.namelist()
    for name in names:
        check_member_name(name)
    if "main.py" not in names:
        raise ApiError("main.py must be at the top of the zip")


async def upload(
    db: DBConnection, blobs: BlobStore, user: User, name: str, game: str, data: bytes
) -> Submission:
    if not 1 <= len(name) <= MAX_NAME:
        raise ApiError(f"name must be 1 to {MAX_NAME} characters")
    check_zip(data)
    id = uuid4()
    await blobs.put(key_for(id), data, "application/zip")
    try:
        await db.submission_repo.insert(
            id, user.id, user.team_id, name, game, len(data), hashlib.sha256(data).hexdigest()
        )
    except Exception:
        await blobs.delete(key_for(id))
        raise
    return await get_submission(db, id)


async def get_submission(db: DBConnection, id: UUID) -> Submission:
    submission = await db.submission_repo.get(id)
    if submission is None:
        raise NotFound(f"no submission {id}")
    return submission


async def list_submissions(db: DBConnection, user: User, mine: bool) -> list[Submission]:
    return await db.submission_repo.list_recent(user.team_id if mine else None)
