"""submissions

Revision ID: 0003
Revises: 0002
Create Date: 2026-09-15
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0003"
down_revision: str | Sequence[str] | None = "0002"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # One row per upload; the zip lives in blob storage under a key derived from the id.
    op.execute("""
        create table submissions (
            id uuid primary key,
            user_id bigint not null references users(id),
            name text not null,
            game text not null,
            size integer not null,
            sha256 text not null,
            created_at timestamptz not null default now()
        )
    """)
    op.execute("create index submissions_user_idx on submissions (user_id, created_at desc)")


def downgrade() -> None:
    op.execute("drop table submissions")
