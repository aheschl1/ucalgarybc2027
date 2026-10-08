"""Participant teams. Every user is on exactly one: creating a team or joining one by its
code moves the caller out of the last, which keeps its submissions."""

import secrets

from psycopg.errors import UniqueViolation

from api.db import DBConnection
from api.errors import Conflict, NotFound
from api.models.teams import MyTeam, TeamElo
from api.models.users import User


def new_code() -> str:
    return secrets.token_urlsafe(12)


async def my_team(db: DBConnection, team_id: int) -> MyTeam:
    team = await db.team_repo.get(team_id)
    if team is None:
        raise NotFound(f"no team {team_id}")
    return MyTeam(**team.model_dump(), members=await db.team_repo.members(team_id))


async def create_team(db: DBConnection, user: User, name: str) -> MyTeam:
    try:
        team = await db.team_repo.insert(name, new_code())
    except UniqueViolation as e:
        raise Conflict(f"a team is already called {name}") from e
    await db.user_repo.set_team(user.id, team.id)
    return await my_team(db, team.id)


async def join_team(db: DBConnection, user: User, code: str) -> MyTeam:
    team = await db.team_repo.get_by_code(code.strip())
    if team is None:
        raise NotFound("no team has that code")
    await db.user_repo.set_team(user.id, team.id)
    return await my_team(db, team.id)


async def replace_code(db: DBConnection, user: User) -> MyTeam:
    """A new join code for the caller's team; the old one stops working."""
    await db.team_repo.set_code(user.team_id, new_code())
    return await my_team(db, user.team_id)


async def list_elos(db: DBConnection) -> list[TeamElo]:
    return await db.team_repo.list_elos()


async def get_elo(db: DBConnection, team_id: int) -> TeamElo:
    team = await db.team_repo.get_elo(team_id)
    if team is None:
        raise NotFound(f"no team {team_id}")
    return team
