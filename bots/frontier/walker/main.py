"""The Phase 0 load test: the commander spawns until the cap, and every worker decodes
its view and walks to a random free neighbour each step."""

import random
from contextlib import suppress

from ucbc.games.frontier import WALKABLE, Coord, Dir, FrontierHandle, Kind, Tiles
from ucbc.handle import ActionError

STEPS = {Dir.UP: (0, -1), Dir.DOWN: (0, 1), Dir.LEFT: (-1, 0), Dir.RIGHT: (1, 0)}

rng: random.Random | None = None


def step(handle: FrontierHandle) -> None:
    global rng
    seen = handle.observe()
    if rng is None:
        rng = random.Random(seen.me.id)
    if seen.me.kind == Kind.COMMANDER:
        # Refused at the cap.
        with suppress(ActionError):
            handle.spawn()
        return
    tiles = Tiles(seen)
    taken = {(u.pos.x, u.pos.y) for u in seen.units}
    me = seen.me.pos
    options = [
        d
        for d, (dx, dy) in STEPS.items()
        if tiles[Coord(me.x + dx, me.y + dy)] in WALKABLE and (me.x + dx, me.y + dy) not in taken
    ]
    if options:
        handle.move(rng.choice(options))
