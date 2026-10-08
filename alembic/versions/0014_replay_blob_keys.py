"""set replays move to the blob store

Revision ID: 0014
Revises: 0013
Create Date: 2026-10-01
"""

import asyncio
from collections.abc import Sequence
from uuid import UUID

from sqlalchemy import text

from alembic import op
from api.blobs import BlobStore
from api.settings import settings

revision: str = "0014"
down_revision: str | Sequence[str] | None = "0013"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None

KEYS = "select s.match_id, s.index, m.attempts from sets s join matches m on m.id = s.match_id"


def key_for(match_id: UUID, attempt: int, index: int) -> str:
    """As api.services.matches.replay_key laid keys out when this ran, frozen here: a later
    change to the layout must not move what old rows point at."""
    return f"matches/{match_id}/attempts/{attempt}/sets/{index}.json.gz"


def upgrade() -> None:
    # A set's gzipped JSON moves from its row to the blob store, and the row keeps the key.
    # SQL cannot reach the store, so the rows move through Python; this runs online only.
    conn = op.get_bind()
    blobs = BlobStore(settings.blob_url)
    op.execute("alter table sets add column replay_key text")

    async def move() -> None:
        await blobs.ensure_bucket()
        for match_id, index, attempts in conn.execute(text(KEYS)).all():
            where = {"m": match_id, "i": index}
            (gz,) = conn.execute(
                text("select replay from sets where match_id = :m and index = :i"), where
            ).one()
            key = key_for(match_id, attempts, index)
            await blobs.put(key, bytes(gz), "application/json", content_encoding="gzip")
            conn.execute(
                text("update sets set replay_key = :k where match_id = :m and index = :i"),
                {"k": key, **where},
            )

    asyncio.run(move())
    op.execute("alter table sets alter column replay_key set not null")
    op.execute("alter table sets drop column replay")


def downgrade() -> None:
    # The replays come back into their rows; the blobs stay where they are.
    conn = op.get_bind()
    blobs = BlobStore(settings.blob_url)
    op.execute("alter table sets add column replay bytea")
    op.execute("alter table sets alter column replay set storage external")

    async def move() -> None:
        for match_id, index, key in conn.execute(
            text("select match_id, index, replay_key from sets")
        ).all():
            conn.execute(
                text("update sets set replay = :gz where match_id = :m and index = :i"),
                {"gz": await blobs.get(key), "m": match_id, "i": index},
            )

    asyncio.run(move())
    op.execute("alter table sets alter column replay set not null")
    op.execute("alter table sets drop column replay_key")
