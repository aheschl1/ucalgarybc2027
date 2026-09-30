"""set replays gzipped

Revision ID: 0010
Revises: 0009
Create Date: 2026-09-30
"""

import gzip
from collections.abc import Sequence

from sqlalchemy import text

from alembic import op

revision: str = "0010"
down_revision: str | Sequence[str] | None = "0009"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None

KEYS = "select match_id, index from sets"


def upgrade() -> None:
    # A set's replay is its JSON gzipped, in a column Postgres does not try to compress
    # again. SQL cannot gzip, so the rows move through Python; this runs online only.
    conn = op.get_bind()
    op.execute("alter table sets add column replay_gz bytea")
    op.execute("alter table sets alter column replay_gz set storage external")
    for match_id, index in conn.execute(text(KEYS)).all():
        key = {"m": match_id, "i": index}
        (replay,) = conn.execute(
            text("select cast(replay as text) from sets where match_id = :m and index = :i"), key
        ).one()
        conn.execute(
            text("update sets set replay_gz = :gz where match_id = :m and index = :i"),
            {"gz": gzip.compress(replay.encode(), compresslevel=6), **key},
        )
    op.execute("alter table sets drop column replay")
    op.execute("alter table sets rename column replay_gz to replay")
    op.execute("alter table sets alter column replay set not null")


def downgrade() -> None:
    conn = op.get_bind()
    op.execute("alter table sets add column replay_json jsonb")
    for match_id, index in conn.execute(text(KEYS)).all():
        key = {"m": match_id, "i": index}
        (gz,) = conn.execute(
            text("select replay from sets where match_id = :m and index = :i"), key
        ).one()
        conn.execute(
            text(
                "update sets set replay_json = cast(:j as jsonb) where match_id = :m and index = :i"
            ),
            {"j": gzip.decompress(gz).decode(), **key},
        )
    op.execute("alter table sets drop column replay")
    op.execute("alter table sets rename column replay_json to replay")
    op.execute("alter table sets alter column replay set not null")
