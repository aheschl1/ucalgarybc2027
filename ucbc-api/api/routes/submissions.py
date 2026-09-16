from pathlib import PurePath
from typing import Annotated
from uuid import UUID

from fastapi import APIRouter, Form, UploadFile, status

from api.auth import CurrentUser
from api.blobs import Blobs
from api.db import DB
from api.errors import PayloadTooLarge
from api.models.submissions import Submission
from api.services import submissions
from api.services.submissions import MAX_ZIP

router = APIRouter(prefix="/submissions", tags=["submissions"])


@router.post("", status_code=status.HTTP_201_CREATED)
async def upload(
    file: UploadFile,
    game: Annotated[str, Form()],
    db: DB,
    blobs: Blobs,
    user: CurrentUser,
    name: Annotated[str | None, Form()] = None,
) -> Submission:
    """Upload a zip with main.py at the top. The name defaults to the file's."""
    data = await file.read(MAX_ZIP + 1)
    if len(data) > MAX_ZIP:
        raise PayloadTooLarge(f"zip is larger than {MAX_ZIP >> 20} MiB")
    name = name or PurePath(file.filename or "bot").stem
    return await submissions.upload(db, blobs, user, name, game, data)


@router.get("")
async def list_submissions(db: DB, user: CurrentUser, mine: bool = False) -> list[Submission]:
    """Every user's submissions, newest first; `mine` narrows to the caller's."""
    return await submissions.list_submissions(db, user, mine)


@router.get("/{submission_id}")
async def get(submission_id: UUID, db: DB, _user: CurrentUser) -> Submission:
    return await submissions.get_submission(db, submission_id)
