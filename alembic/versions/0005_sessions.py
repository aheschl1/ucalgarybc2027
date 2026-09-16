"""sessions

Revision ID: 0005
Revises: 0004
Create Date: 2026-09-15
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0005"
down_revision: str | Sequence[str] | None = "0004"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A logged-in browser holds the token in a cookie; only its hash is stored.
    op.execute("""
        create table sessions (
            token_hash text primary key,
            user_id bigint not null references users(id) on delete cascade,
            created_at timestamptz not null default now(),
            expires_at timestamptz not null
        )
    """)
    op.execute("create index sessions_user_id on sessions (user_id)")


def downgrade() -> None:
    op.execute("drop table sessions")
