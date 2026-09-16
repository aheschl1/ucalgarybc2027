"""match origin

Revision ID: 0004
Revises: 0003
Create Date: 2026-09-15
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0004"
down_revision: str | Sequence[str] | None = "0003"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A platform match (from a schedule) is visible to everyone; a user match only to the
    # owners of the submissions in it. Rows from before this revision count as user matches.
    op.execute("""
        alter table matches
            add column origin text not null default 'user'
                check (origin in ('user', 'platform'))
    """)
    op.execute("alter table matches alter column origin drop default")


def downgrade() -> None:
    op.execute("alter table matches drop column origin")
