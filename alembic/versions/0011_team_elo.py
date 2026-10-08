"""team elo

Revision ID: 0011
Revises: 0010
Create Date: 2026-10-03
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0011"
down_revision: str | Sequence[str] | None = "0010"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A float, so rounding never leaks points; `elo_matches` counts the matches that moved it.
    op.execute("alter table teams add column elo double precision not null default 800")
    op.execute("alter table teams add column elo_matches integer not null default 0")


def downgrade() -> None:
    op.execute("alter table teams drop column elo_matches")
    op.execute("alter table teams drop column elo")
