from datetime import datetime
from typing import Any

import click
import httpx

from ucbc.cli.session import auth_headers, check_status
from ucbc.settings import server_url

def _widths(rows: list[dict[str, Any]]) -> dict[str, int]:
    """The widest value in each padded column, over every row."""
    return {key: max(len(row[key]) for row in rows) for key in ("display_name", "name", "game")}

def _line(row: dict[str, Any], widths: dict[str, int]) -> str:
    """One submission: team, bot, game, upload time, id."""
    team = row["display_name"]
    bot = row["name"]
    game = row["game"]
    sub_id = row["id"]

    # formats the upload date
    created_at = datetime.fromisoformat(row["created_at"]).astimezone().strftime("%Y-%m-%d %H:%M")

    return (
        f"{team:<{widths['display_name']}}  "
        f"{bot:<{widths['name']}}  "
        f"{game:<{widths['game']}}  "
        f"{created_at}  "
        f"{sub_id}  "
    )

@click.command()
@click.option("--team", help="Only this team's bots")
@click.option("--all", "all_teams", is_flag=True, help="Every team's bots")
@click.option("--limit", type=click.IntRange(min=1), help="Show only the N most recent")
def submissions(team: str | None, all_teams: bool, limit: int | None) -> None:
    """Lists submissions, most recent first: your team's, --team's, or --all."""
    if team and all_teams:
        raise click.UsageError(
            f"--team {team} and --all were both used. Pick one team or --all only"
        )
    headers = auth_headers()
    if not headers:
        raise click.ClickException("no user logged in: run ucbc login")

    params = {"mine": "true"}
    if team or all_teams:
        params = {}
    try:
        response = httpx.get(f"{server_url()}/submissions", headers=headers, params=params) 
        check_status(response)
    except httpx.HTTPError as e:
        raise click.ClickException(f"could not reach {server_url()}: {e}. Try again") from e   

    rows: list[dict[str, Any]] = response.json()
    # filter by team name
    if team:
        rows = [row for row in rows if row["display_name"].casefold() == team.casefold()]

    if not rows:
        if all_teams:
            raise click.ClickException("No bots have been submitted yet")        
        elif team:
            raise click.ClickException(f"Team {team} has no bots submitted yet")        
        else:
            raise click.ClickException("Your team has no bots submitted yet, upload your bot using 'ucbc submit'")

    if limit is not None:
        rows = rows[:limit]

    widths = _widths(rows)
    text = "\n".join(_line(row, widths) for row in rows)
    if limit is None:
        click.echo_via_pager(text)
    else:
        click.echo(text)
