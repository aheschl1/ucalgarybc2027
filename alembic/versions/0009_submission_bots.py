"""matches name submissions only

Revision ID: 0009
Revises: 0008
Create Date: 2026-09-26
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0009"
down_revision: str | Sequence[str] | None = "0008"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A bot directory on the worker host has no team, so matches with one are dropped rather
    # than kept without an owner. `bots` becomes the submission ids in slot order.
    op.execute("""
        delete from matches where exists (
            select 1 from jsonb_array_elements(bots) b where b->>'kind' = 'path'
        )
    """)
    op.execute("""
        update matches set bots = coalesce(
            (select jsonb_agg(b->'id' order by n)
             from jsonb_array_elements(bots) with ordinality e(b, n)),
            '[]'::jsonb
        )
    """)


def downgrade() -> None:
    op.execute("""
        update matches set bots = coalesce(
            (select jsonb_agg(jsonb_build_object('kind', 'submission', 'id', id) order by n)
             from jsonb_array_elements(bots) with ordinality e(id, n)),
            '[]'::jsonb
        )
    """)
