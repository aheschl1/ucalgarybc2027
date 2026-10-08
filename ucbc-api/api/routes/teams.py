from fastapi import APIRouter, status

from api.auth import CurrentUser
from api.db import DB
from api.models.teams import MyTeam, TeamCreate, TeamElo, TeamJoin
from api.services import teams

router = APIRouter(prefix="/teams", tags=["teams"])


@router.get("/me")
async def me(db: DB, user: CurrentUser) -> MyTeam:
    return await teams.my_team(db, user.team_id)


@router.post("", status_code=status.HTTP_201_CREATED)
async def create(body: TeamCreate, db: DB, user: CurrentUser) -> MyTeam:
    """Start a team and move into it. The last team keeps its submissions."""
    return await teams.create_team(db, user, body.name)


@router.post("/join")
async def join(body: TeamJoin, db: DB, user: CurrentUser) -> MyTeam:
    """Move into the team with this join code. The last team keeps its submissions."""
    return await teams.join_team(db, user, body.code)


@router.post("/me/code")
async def replace_code(db: DB, user: CurrentUser) -> MyTeam:
    """A new join code; the old one stops working."""
    return await teams.replace_code(db, user)


@router.get("/elo")
async def list_elos(db: DB) -> list[TeamElo]:
    """Every team's rating, highest first. Public."""
    return await teams.list_elos(db)


@router.get("/{team_id}/elo")
async def get_elo(team_id: int, db: DB) -> TeamElo:
    """One team's rating. Public."""
    return await teams.get_elo(db, team_id)
