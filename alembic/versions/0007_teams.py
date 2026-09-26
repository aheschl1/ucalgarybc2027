"""teams

Revision ID: 0007
Revises: 0006
Create Date: 2026-09-25
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0007"
down_revision: str | Sequence[str] | None = "0006"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None


def upgrade() -> None:
    # A participant team owns submissions; a user is on exactly one. Not the engine's
    # in-game teams.
    op.execute("""
        create table teams (
            id bigint generated always as identity primary key,
            name text not null,
            created_at timestamptz not null default now(),
            seed_user_id bigint
        )
    """)
    op.execute("create unique index teams_name_key on teams (lower(name))")
    # Every existing user gets a team named after them. The oldest of a clashing display
    # name keeps it, the rest are numbered. `seed_user_id` maps teams back to users.
    op.execute("""
        insert into teams (name, seed_user_id)
        select case when n = 1 then display_name else display_name || ' ' || n end, id
        from (
            select id, display_name,
                row_number() over (partition by lower(display_name) order by id) as n
            from users
        ) u
    """)
    op.execute("alter table users add column team_id bigint references teams(id)")
    op.execute("update users set team_id = t.id from teams t where t.seed_user_id = users.id")
    op.execute("alter table teams drop column seed_user_id")
    op.execute("alter table users alter column team_id set not null")

    # A submission belongs to its uploader's team; `user_id` stays as the uploader.
    op.execute("alter table submissions add column team_id bigint references teams(id)")
    op.execute("update submissions s set team_id = u.team_id from users u where u.id = s.user_id")
    op.execute("alter table submissions alter column team_id set not null")
    op.execute("create index submissions_team_idx on submissions (team_id, created_at desc)")


def downgrade() -> None:
    op.execute("alter table submissions drop column team_id")
    op.execute("alter table users drop column team_id")
    op.execute("drop table teams")
