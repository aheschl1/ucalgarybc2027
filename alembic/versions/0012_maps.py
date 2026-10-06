"""maps

Revision ID: 0012
Revises: 0011
Create Date: 2026-10-04
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0012"
down_revision: str | Sequence[str] | None = "0011"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A map file is in the bucket at `maps/<id>.map`; the row is what the platform knows of
    # it. Matches name maps by id, so a map is archived, never deleted.
    op.execute("""
        create table maps (
            id uuid primary key,
            game text not null,
            name text not null,
            size integer not null,
            sha256 text not null,
            uploaded_by bigint references users(id) on delete set null,
            created_at timestamptz not null default now(),
            archived_at timestamptz
        )
    """)
    op.execute("create unique index maps_game_name_idx on maps (game, lower(name))")
    # Map ids in set order: one plays every set, otherwise one per set. Empty is the game's
    # standard map, as for every match before this revision.
    op.execute("alter table matches add column maps jsonb not null default '[]'::jsonb")
    op.execute("alter table matches alter column maps drop default")


def downgrade() -> None:
    op.execute("alter table matches drop column maps")
    op.execute("drop table maps")
