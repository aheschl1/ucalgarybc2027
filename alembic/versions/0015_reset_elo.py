"""reset elo

Revision ID: 0015
Revises: 0014
Create Date: 2026-10-13
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0015"
down_revision: str | Sequence[str] | None = "0014"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # Ratings start over: every team goes back to the `teams.elo` default as if it had never
    # played, and the platform matches that moved them go, sets and all. Their replay blobs
    # stay in the store, orphaned; nothing points at them any more.
    op.execute("delete from matches where origin = 'platform'")
    op.execute("update teams set elo = default, elo_matches = default")


def downgrade() -> None:
    # The deleted matches and old ratings are gone for good.
    pass
