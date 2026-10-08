"""Team ratings: the step itself, and the worker applying it when a match finishes."""

from collections.abc import Callable
from pathlib import Path
from uuid import UUID

import pytest
from httpx import AsyncClient

from api.blobs import BlobStore
from api.db import DBConnection
from api.models.matches import MatchReplay
from api.services import elo, matches
from tests.api.test_queue import LEASE, enqueue, set_replay
from tests.api.test_submissions import upload, zip_dir

# Team ids: signup made the admin's team first, then the member's.
ROOT, ALICE = 1, 2


def test_expected_is_even_for_equal_ratings() -> None:
    assert elo.expected(800, 800) == 0.5
    assert elo.expected(1200, 800) == pytest.approx(10 / 11)
    assert elo.expected(1200, 800) + elo.expected(800, 1200) == pytest.approx(1)


def test_score_is_the_share_of_sets() -> None:
    assert elo.score([3, 0], 3) == 1
    assert elo.score([0, 3], 3) == 0
    assert elo.score([2, 1], 3) == pytest.approx(2 / 3)
    # A drawn set is half to each side.
    assert elo.score([1, 1], 3) == 0.5
    assert elo.score([2, 0], 3) == pytest.approx(5 / 6)


def test_step_moves_points_between_the_two() -> None:
    a, b = elo.step(800, 800, 1)
    assert (a, b) == (816, 784)
    a, b = elo.step(1000, 800, 0.5)
    assert a < 1000 and b > 800
    assert a + b == pytest.approx(1800)


def replay(match_id: UUID, winners: list[int | None]) -> MatchReplay:
    sets = [set_replay(i, w) for i, w in enumerate(winners)]
    set_wins = [sum(1 for w in winners if w == team) for team in (0, 1)]
    return MatchReplay(
        match_id=str(match_id),
        engine_version="0.1.0",
        config={},
        teams=[{"id": 0, "name": "a"}, {"id": 1, "name": "b"}],
        sets=sets,
        result={
            "sets": [s["result"] for s in sets],
            "set_wins": set_wins,
            "winner_team": None if set_wins[0] == set_wins[1] else int(set_wins[1] > set_wins[0]),
        },
    )


async def ratings(db: DBConnection) -> dict[int, tuple[float, int]]:
    cur = await db.conn.execute("select id, elo, elo_matches from teams order by id")
    return {row["id"]: (row["elo"], row["elo_matches"]) for row in await cur.fetchall()}


@pytest.fixture
async def bots(
    admin_client: AsyncClient, member_client: AsyncClient, bot: Callable[[str], Path]
) -> dict[int, str]:
    """One submission per team, by team id."""
    ids = {}
    for team, client in [(ROOT, admin_client), (ALICE, member_client)]:
        r = await upload(client, zip_dir(bot("random")), form={"name": f"team{team}"})
        assert r.json()["team_id"] == team
        ids[team] = r.json()["id"]
    return ids


async def play(
    client: AsyncClient,
    db: DBConnection,
    blobs: BlobStore,
    bots: list[str],
    winners: list[int | None],
) -> None:
    """Queues the match, claims it, and finishes it with these set winners."""
    match_id = await enqueue(client, {"game": "tictactoe", "bots": bots})
    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None and held.id == match_id
    await matches.finish_match(db, held, replay(match_id, winners), blobs)


async def test_new_teams_start_at_800(db: DBConnection, bots: dict[int, str]) -> None:
    assert await ratings(db) == {ROOT: (800, 0), ALICE: (800, 0)}


async def test_a_finished_match_moves_both_teams(
    admin_client: AsyncClient, db: DBConnection, blobs: BlobStore, bots: dict[int, str]
) -> None:
    await play(admin_client, db, blobs, [bots[ROOT], bots[ALICE]], [0, 0, 0])
    assert await ratings(db) == {ROOT: (816, 1), ALICE: (784, 1)}

    # Alice in slot 0 wins 2-1, which scores her two thirds.
    await play(admin_client, db, blobs, [bots[ALICE], bots[ROOT]], [0, 1, 0])
    alice, root = elo.step(784, 816, 2 / 3)
    after = await ratings(db)
    assert [after[ROOT][0], after[ALICE][0]] == pytest.approx([root, alice])
    assert after[ROOT][1] == after[ALICE][1] == 2
    assert alice - 784 < elo.step(784, 816, 1)[0] - 784


async def test_an_even_split_lifts_the_lower_team(
    admin_client: AsyncClient, db: DBConnection, blobs: BlobStore, bots: dict[int, str]
) -> None:
    await db.conn.execute("update teams set elo = 1000 where id = %s", (ROOT,))
    await play(admin_client, db, blobs, [bots[ROOT], bots[ALICE]], [0, None, 1])
    root, alice = elo.step(1000, 800, 0.5)
    assert root < 1000 and alice > 800
    after = await ratings(db)
    assert [after[ROOT][0], after[ALICE][0]] == pytest.approx([root, alice])


async def test_self_play_and_errors_rate_nothing(
    admin_client: AsyncClient, db: DBConnection, blobs: BlobStore, bots: dict[int, str]
) -> None:
    await play(admin_client, db, blobs, [bots[ROOT], bots[ROOT]], [0, 0, 0])

    match_id = await enqueue(admin_client, {"game": "tictactoe", "bots": list(bots.values())})
    held = await matches.claim_match(db, "w", LEASE)
    assert held is not None and held.id == match_id
    await matches.fail_match(db, held, "boom")

    assert await ratings(db) == {ROOT: (800, 0), ALICE: (800, 0)}


async def test_a_lost_lease_rates_nothing(
    admin_client: AsyncClient, db: DBConnection, blobs: BlobStore, bots: dict[int, str]
) -> None:
    match_id = await enqueue(admin_client, {"game": "tictactoe", "bots": list(bots.values())})
    held = await matches.claim_match(db, "w1", LEASE)
    assert held is not None
    await db.conn.execute(
        "update matches set heartbeat_at = now() - interval '1 hour' where id = %s", (match_id,)
    )
    taken = await matches.claim_match(db, "w2", LEASE)
    assert taken is not None

    with pytest.raises(matches.LostLease):
        await matches.finish_match(db, held, replay(match_id, [0, 0, 0]), blobs)
    assert await ratings(db) == {ROOT: (800, 0), ALICE: (800, 0)}

    # The holder's result counts once.
    await matches.finish_match(db, taken, replay(match_id, [1, 1, 1]), blobs)
    assert await ratings(db) == {ROOT: (784, 1), ALICE: (816, 1)}


async def test_anyone_reads_ratings(
    admin_client: AsyncClient,
    client: AsyncClient,
    db: DBConnection,
    blobs: BlobStore,
    bots: dict[int, str],
) -> None:
    await play(admin_client, db, blobs, [bots[ROOT], bots[ALICE]], [1, 1, 0])
    alice, root = elo.step(800, 800, 2 / 3)

    # `client` is signed out. Ratings are rounded; join codes stay hidden.
    r = await client.get("/teams/elo")
    assert r.status_code == 200, r.text
    assert r.json() == [
        {"id": ALICE, "name": "Alice", "elo": round(alice), "matches": 1},
        {"id": ROOT, "name": "Root", "elo": round(root), "matches": 1},
    ]
    r = await client.get(f"/teams/{ROOT}/elo")
    assert r.json() == {"id": ROOT, "name": "Root", "elo": round(root), "matches": 1}
    assert (await client.get("/teams/99/elo")).status_code == 404
