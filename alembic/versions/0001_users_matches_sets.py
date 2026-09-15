"""users, matches, sets

Revision ID: 0001
Revises:
Create Date: 2026-09-14
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0001"
down_revision: str | Sequence[str] | None = None
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    op.execute("""
        create table users (
            id bigint generated always as identity primary key,
            username text not null unique,
            password_hash text not null,
            is_admin boolean not null default false,
            created_at timestamptz not null default now()
        )
    """)
    op.execute("""
        create table matches (
            id uuid primary key default gen_random_uuid(),
            game text not null,
            engine_version text not null,
            teams jsonb not null,
            config jsonb not null,
            status text not null check (status in ('running', 'done')),
            set_wins jsonb,
            winner_team integer,
            created_at timestamptz not null default now(),
            completed_at timestamptz
        )
    """)
    op.execute("""
        create table sets (
            match_id uuid not null references matches(id) on delete cascade,
            index integer not null,
            first_team integer not null,
            winner_team integer,
            reason text not null,
            detail text not null default '',
            ticks integer not null,
            replay jsonb not null,
            created_at timestamptz not null default now(),
            primary key (match_id, index)
        )
    """)


def downgrade() -> None:
    op.execute("drop table sets")
    op.execute("drop table matches")
    op.execute("drop table users")
