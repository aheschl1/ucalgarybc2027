"""Platform matches: each pair of teams plays their newest submissions once. A new upload
is a new pairing with every other team's newest, so the ladder follows what teams ship."""

import random
from itertools import combinations, groupby
from uuid import UUID

from api.db import DBConnection
from api.models.matches import MatchConfig, TeamInfo

# Below every user match, so a burst of pairings does not hold up what teams queue themselves.
PRIORITY = -1


async def schedule_pending(
    db: DBConnection, sets: int, game: str | None = None, rng: random.Random | None = None
) -> list[UUID]:
    """Queues a platform match of `sets` sets for each pair of teams whose newest submissions
    have not played each other, in every game or in `game`, and returns the new matches'
    ids. Each set plays one of the game's unarchived maps at random; a game without maps
    plays its standard one. Several workers may run this at once: the pairs are found and
    queued under a lock, so the next holder sees what the last one queued."""
    rng = rng or random.Random()
    config = MatchConfig(sets=sets)
    ids = []
    async with db.conn.transaction():
        await db.match_repo.lock_scheduling() # we dont want to interuspt an in-progress scheduling. what if we scale horizontally?
        latest = await db.submission_repo.list_latest_per_team(game)
        played = {
            frozenset(bots)
            for bots in await db.match_repo.list_bots_among([s.id for s in latest], "platform")
        }
        for g, submissions in groupby(latest, key=lambda s: s.game):
            maps = [m.id for m in await db.map_repo.list_maps(g) if m.archived_at is None]
            for a, b in combinations(submissions, 2):
                if frozenset((a.id, b.id)) in played:
                    continue
                teams = [TeamInfo(id=0, name=a.name), TeamInfo(id=1, name=b.name)]
                ids.append(
                    await db.match_repo.insert(
                        "platform",
                        g,
                        teams,
                        config,
                        [a.id, b.id],
                        pick_maps(maps, sets, rng),
                        PRIORITY,
                    )
                )
    return ids


def pick_maps(maps: list[UUID], sets: int, rng: random.Random) -> list[UUID]:
    """One map per set: distinct while there are enough, repeats otherwise, none when there
    are none."""
    if not maps:
        return []
    if len(maps) >= sets:
        return rng.sample(maps, sets)
    return rng.choices(maps, k=sets)
