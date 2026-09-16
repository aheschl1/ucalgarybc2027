"""match queue

Revision ID: 0002
Revises: 0001
Create Date: 2026-09-15
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0002"
down_revision: str | Sequence[str] | None = "0001"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A match is the queue entry: queued until a worker claims it, error if it never
    # finishes. `bots` names the code to run; matches from before this revision get an
    # empty list. The engine version is known once the match has run.
    op.execute("""
        alter table matches
            drop constraint matches_status_check,
            add constraint matches_status_check
                check (status in ('queued', 'running', 'done', 'error')),
            alter column engine_version drop not null,
            add column error text,
            add column bots jsonb not null default '[]'::jsonb,
            add column priority integer not null default 0,
            add column attempts integer not null default 0,
            add column max_attempts integer not null default 3,
            add column claimed_by text,
            add column claimed_at timestamptz,
            add column heartbeat_at timestamptz
    """)
    op.execute("alter table matches alter column bots drop default")
    op.execute("""
        create index matches_queue_idx on matches (priority desc, created_at)
            where status in ('queued', 'running')
    """)


def downgrade() -> None:
    op.execute("drop index matches_queue_idx")
    op.execute("""
        alter table matches
            drop column heartbeat_at,
            drop column claimed_at,
            drop column claimed_by,
            drop column max_attempts,
            drop column attempts,
            drop column priority,
            drop column bots,
            drop column error,
            alter column engine_version set not null,
            drop constraint matches_status_check,
            add constraint matches_status_check check (status in ('running', 'done'))
    """)
