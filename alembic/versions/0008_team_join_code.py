"""team join code

Revision ID: 0008
Revises: 0007
Create Date: 2026-09-25
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0008"
down_revision: str | Sequence[str] | None = "0007"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # Whoever has the code can join the team. Members read it back, so it is kept as is.
    op.execute("alter table teams add column join_code text")
    op.execute("update teams set join_code = replace(gen_random_uuid()::text, '-', '')")
    op.execute("alter table teams alter column join_code set not null")
    op.execute("alter table teams add constraint teams_join_code_key unique (join_code)")


def downgrade() -> None:
    op.execute("alter table teams drop column join_code")
