"""set replays move to blob storage

Revision ID: 0014
Revises: 0013
Create Date: 2026-10-01
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0014"
down_revision: str | Sequence[str] | None = "0013"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # New sets store their replay in the blob store and keep only its key here, so the
    # old inline column stops being required.
    op.execute("alter table sets add column replay_key text")
    op.execute("alter table sets alter column replay drop not null")


def downgrade() -> None:
    op.execute("alter table sets drop column replay_key")
    op.execute("alter table sets alter column replay set not null")
