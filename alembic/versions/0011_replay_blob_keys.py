"""store set replay pointers

Revision ID: 0011
Revises: 0010
Create Date: 2026-10-01
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0011"
down_revision: str | Sequence[str] | None = "0010"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
	op.execute("alter table sets add column replay_key text")
	# make the old replay column nullable, so we can store the replay in the blob store
	op.execute("alter table sets alter column replay drop not null")


def downgrade() -> None:
	op.execute("alter table sets drop column replay_key")
	op.execute("alter table sets alter column replay set not null")
