"""The platform pairs each team's newest submission with every other team's, once."""

import random
from collections.abc import Callable
from pathlib import Path
from uuid import UUID

from httpx import AsyncClient

from api.db import DBConnection
from api.services import scheduling
from tests.api.test_maps import MAPS
from tests.api.test_maps import upload as upload_map
from tests.api.test_queue import get
from tests.api.test_submissions import upload, zip_dir
from tests.api.test_worker import UCBC2027_BOTS

BotPath = Callable[[str], Path]


async def submit(client: AsyncClient, path: Path, game: str = "tictactoe") -> UUID:
    r = await upload(client, zip_dir(path), form={"name": path.name, "game": game})
    assert r.status_code == 201, r.text
    return UUID(r.json()["id"])


async def test_newest_submissions_pair_once(
    member_client: AsyncClient,
    member_b_client: AsyncClient,
    teammate_client: AsyncClient,
    db: DBConnection,
    bot: BotPath,
) -> None:
    assert await scheduling.schedule_pending(db, sets=3) == []
    a = await submit(member_client, bot("random"))
    assert await scheduling.schedule_pending(db, sets=3) == []
    b = await submit(member_b_client, bot("first_empty"))

    [match_id] = await scheduling.schedule_pending(db, sets=3)
    match = await get(member_b_client, match_id)
    assert match["origin"] == "platform"
    assert match["bots"] == [str(a), str(b)]
    assert [t["name"] for t in match["teams"]] == ["random", "first_empty"]
    assert match["config"]["sets"] == 3
    assert match["maps"] == []
    assert match["priority"] == scheduling.PRIORITY
    assert await scheduling.schedule_pending(db, sets=3) == []

    # A teammate's upload is the team's newest: one new pairing, with the other team.
    c = await submit(teammate_client, bot("random"))
    [match_id] = await scheduling.schedule_pending(db, sets=3)
    assert (await get(member_client, match_id))["bots"] == [str(c), str(b)]
    assert await scheduling.schedule_pending(db, sets=3) == []


async def test_user_matches_do_not_count(
    member_client: AsyncClient, member_b_client: AsyncClient, db: DBConnection, bot: BotPath
) -> None:
    a = await submit(member_client, bot("random"))
    b = await submit(member_b_client, bot("first_empty"))
    body = {"game": "tictactoe", "bots": [str(b), str(a)]}
    assert (await member_client.post("/matches/queue", json=body)).status_code == 201

    assert len(await scheduling.schedule_pending(db, sets=1)) == 1


async def test_games_pair_separately(
    member_client: AsyncClient, member_b_client: AsyncClient, db: DBConnection, bot: BotPath
) -> None:
    await submit(member_client, bot("random"))
    await submit(member_b_client, UCBC2027_BOTS / "noop", "ucbc2027")
    assert await scheduling.schedule_pending(db, sets=1) == []

    await submit(member_b_client, bot("first_empty"))
    await submit(member_client, UCBC2027_BOTS / "noop", "ucbc2027")
    games = [
        (await get(member_client, id))["game"] for id in await scheduling.schedule_pending(db, 1)
    ]
    assert sorted(games) == ["tictactoe", "ucbc2027"]
    assert await scheduling.schedule_pending(db, sets=1, game="tictactoe") == []


async def test_sets_play_random_unarchived_maps(
    admin_client: AsyncClient,
    member_client: AsyncClient,
    member_b_client: AsyncClient,
    db: DBConnection,
) -> None:
    ids = []
    for name in ["standard", "medium"]:
        r = await upload_map(admin_client, (MAPS / f"{name}.map").read_bytes(), f"{name}.map")
        ids.append(r.json()["id"])
    archived = (await upload_map(admin_client, filename="old.map")).json()["id"]
    assert await db.map_repo.set_archived(UUID(archived), True)
    await submit(member_client, UCBC2027_BOTS / "noop", "ucbc2027")
    await submit(member_b_client, UCBC2027_BOTS / "noop", "ucbc2027")

    [match_id] = await scheduling.schedule_pending(db, sets=3, rng=random.Random(1))
    maps = (await get(member_client, match_id))["maps"]
    assert len(maps) == 3
    assert set(maps) == set(ids)


def test_pick_maps() -> None:
    rng = random.Random(0)
    ids = [UUID(int=i) for i in range(3)]
    assert scheduling.pick_maps([], 3, rng) == []
    assert sorted(scheduling.pick_maps(ids, 3, rng)) == ids
    assert set(scheduling.pick_maps(ids, 5, rng)) <= set(ids)
    assert len(scheduling.pick_maps(ids, 5, rng)) == 5
