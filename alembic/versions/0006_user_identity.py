"""user identity

Revision ID: 0006
Revises: 0005
Create Date: 2026-09-15
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0006"
down_revision: str | Sequence[str] | None = "0005"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # Email replaces the username as the login credential; the display name is the only
    # thing another user sees. Rows from before this revision get a placeholder address
    # and keep their old username as a name.
    op.execute("alter table users add column email text, add column display_name text")
    op.execute("update users set email = username || '@invalid', display_name = username")
    op.execute(
        "alter table users alter column email set not null, alter column display_name set not null"
    )
    op.execute("alter table users add constraint users_email_key unique (email)")
    op.execute("alter table users drop column username")


def downgrade() -> None:
    op.execute("alter table users add column username text")
    op.execute("update users set username = split_part(email, '@', 1)")
    op.execute("alter table users alter column username set not null")
    op.execute("alter table users add constraint users_username_key unique (username)")
    op.execute("alter table users drop column email, drop column display_name")
