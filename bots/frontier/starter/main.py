"""A starting point. The commander spawns workers while the base has wood and spare food. Every
third worker farms wheat in the watered ground around the pond north of the base; the
rest cut wood. Units head home when full or about to go hungry, unload, and take food.
Each unit runs its own copy of this file, so the module globals belong to one unit."""

import random
from collections import deque
from collections.abc import Callable, Iterable
from contextlib import suppress

from ucbc.games.frontier import (
    STEPS,
    WALKABLE,
    Coord,
    Dir,
    FrontierHandle,
    Item,
    Kind,
    Observation,
    Tiles,
    crops,
)
from ucbc.handle import ActionError

CARRY = 20
GRASS, FOREST, DIRT, FARMLAND = 0, 1, 4, 8
FOOD = (Item.APPLE, Item.WHEAT)
PACKED = 3
"""Food a unit takes from the base to carry."""
SEEDS = 4
"""Seeds a farmer takes from the base to carry."""

Pos = tuple[int, int]

rng = random.Random()
# The walk being followed, its next tile last.
path: list[Pos] = []
# Steps to wander before looking for forest again, after a look found none.
wander = 0
# A farmer doesn't go back for seeds before this tick, after the base had none.
seedless_until = 0


def step(handle: FrontierHandle) -> None:
    seen = handle.observe()
    carried = {s.item: s.count for s in seen.me.carrying}
    food = sum(carried.get(f, 0) for f in FOOD)
    if seen.me.kind == Kind.COMMANDER:
        commander(handle, seen, food)
        return
    rng.seed(seen.me.id * 7919 + seen.tick)
    tiles = Tiles(seen)
    me = (seen.me.pos.x, seen.me.pos.y)
    farmer = seen.me.id % 3 == 0
    hungry = food == 0 and seen.me.hungry_at - seen.tick < 80
    full = sum(carried.values()) >= CARRY or (farmer and carried.get(Item.WHEAT, 0) >= 8 + PACKED)
    needs_seeds = farmer and not carried.get(Item.WHEAT_SEED) and seen.tick >= seedless_until
    if hungry or full or needs_seeds:
        home(handle, seen, tiles, me, carried, farmer)
    elif farmer:
        farm(handle, seen, tiles, me, carried)
    else:
        chop(handle, seen, tiles, me)


def commander(handle: FrontierHandle, seen: Observation, food: int) -> None:
    """Keep fed from the base; otherwise spawn while the base has food to spare."""
    if not seen.stores:
        return
    stored = {s.item: s.count for s in seen.stores[0].stored}
    with suppress(ActionError):
        if food < PACKED and stored.get(Item.WHEAT):
            handle.take(toward(seen, seen.stores[0].pos), Item.WHEAT, PACKED)
        elif stored.get(Item.WHEAT, 0) >= 15:
            handle.spawn()


def toward(seen: Observation, to: Coord) -> Dir:
    """The direction of the neighbouring tile `to`."""
    me = seen.me.pos
    return next(d for d, (dx, dy) in STEPS.items() if (me.x + dx, me.y + dy) == (to.x, to.y))


def home(
    handle: FrontierHandle,
    seen: Observation,
    tiles: Tiles,
    me: Pos,
    carried: dict[Item, int],
    farmer: bool,
) -> None:
    """Beside the base, unload one kind of item or take food or seeds; else head there."""
    base = (seen.base.x, seen.base.y)
    keep = {Item.WHEAT: PACKED, Item.WHEAT_SEED: SEEDS if farmer else 0}
    if seen.stores:
        d = toward(seen, seen.stores[0].pos)
        stored = {s.item: s.count for s in seen.stores[0].stored}
        # Into the base, or onto the ground beside it when the base is full.
        for item, n in carried.items():
            for to in [d, *STEPS]:
                if n > keep.get(item, 0):
                    with suppress(ActionError):
                        handle.put(to, item, n - keep.get(item, 0))
                        return
        for item, want in keep.items():
            if carried.get(item, 0) < want and stored.get(item):
                with suppress(ActionError):
                    handle.take(d, item, want - carried.get(item, 0))
                    return
        if farmer and not stored.get(Item.WHEAT_SEED):
            global seedless_until
            seedless_until = seen.tick + 60
        path.clear()
        return
    if not follow(handle, me):
        plan(seen, tiles, me, beside(tiles, [base]))
        if not follow(handle, me):
            towards(handle, me, base)


def farm(
    handle: FrontierHandle, seen: Observation, tiles: Tiles, me: Pos, carried: dict[Item, int]
) -> None:
    """Harvest what's ripe, plant empty farmland, till the rest of the field."""
    bx, by = seen.base.x, seen.base.y
    field = [(x, y) for y in range(by - 10, by - 1) for x in range(bx - 4, bx + 5)]
    growing = {(c.x, c.y): c.ripe_at for c in crops(seen)}
    ripe = [at for at in field if at in growing and growing[at] <= seen.tick]
    seeds = carried.get(Item.WHEAT_SEED, 0)
    empty = [at for at in field if seeds and tiles[at] == FARMLAND and at not in growing]
    wild = [at for at in field if tiles[at] in (GRASS, DIRT)]
    jobs: list[tuple[list[Pos], Callable[[Dir], object]]] = [
        (ripe, handle.harvest),
        (empty, lambda d: handle.plant(d, Item.WHEAT_SEED)),
        (wild, handle.till),
    ]
    for targets, act in jobs:
        for d, (dx, dy) in STEPS.items():
            if (me[0] + dx, me[1] + dy) in targets:
                with suppress(ActionError):
                    act(d)
                path.clear()
                return
    if not follow(handle, me):
        plan(seen, tiles, me, beside(tiles, ripe or empty or wild))
        follow(handle, me)


def chop(handle: FrontierHandle, seen: Observation, tiles: Tiles, me: Pos) -> None:
    """Gather from forest beside, or walk to some in view, or wander."""
    global wander
    for d, (dx, dy) in STEPS.items():
        if tiles[(me[0] + dx, me[1] + dy)] == FOREST:
            handle.gather(d)
            path.clear()
            return
    if follow(handle, me):
        return
    if wander:
        wander -= 1
    else:
        w, ox, oy = tiles.width, tiles.origin.x, tiles.origin.y
        forest = [(i % w + ox, i // w + oy) for i, t in enumerate(tiles.data) if t == FOREST]
        plan(seen, tiles, me, beside(tiles, forest))
        if follow(handle, me):
            return
        wander = 10
    towards(handle, me, (me[0] + rng.randint(-9, 9), me[1] + rng.randint(-9, 9)))


def beside(tiles: Tiles, targets: Iterable[Pos]) -> bytearray:
    """A goal for `plan`: the tiles in view next to any of `targets`."""
    w, h, ox, oy = tiles.width, tiles.height, tiles.origin.x, tiles.origin.y
    goal = bytearray(w * h)
    for tx, ty in targets:
        for dx, dy in STEPS.values():
            x, y = tx + dx - ox, ty + dy - oy
            if 0 <= x < w and 0 <= y < h:
                goal[y * w + x] = 1
    return goal


def plan(seen: Observation, tiles: Tiles, me: Pos, goal: bytearray) -> None:
    """A shortest walk through the view to a tile marked in `goal` (indexed as the
    view's tiles), into `path`."""
    w, h, ox, oy = tiles.width, tiles.height, tiles.origin.x, tiles.origin.y
    open_ = bytearray(t in WALKABLE for t in tiles.data)
    for rows, width in ((seen.units, 6), (seen.buildings, 7)):
        for i in range(0, len(rows), width):
            open_[(rows[i + 4] - oy) * w + rows[i + 3] - ox] = 0
    start = (me[1] - oy) * w + me[0] - ox
    prev = {start: -1}
    queue = deque([start])
    path.clear()
    while queue:
        i = queue.popleft()
        if i != start and goal[i]:
            while i != start:
                path.append((i % w + ox, i // w + oy))
                i = prev[i]
            return
        x = i % w
        for j in (
            i - w if i >= w else -1,
            i + w if i + w < w * h else -1,
            i - 1 if x else -1,
            i + 1 if x + 1 < w else -1,
        ):
            if j >= 0 and open_[j] and j not in prev:
                prev[j] = i
                queue.append(j)


def follow(handle: FrontierHandle, me: Pos) -> bool:
    """Takes the next step of `path` from `me`; False if there is none or it's blocked."""
    if not path:
        return False
    for d, (dx, dy) in STEPS.items():
        if (me[0] + dx, me[1] + dy) == path[-1]:
            try:
                handle.move(d)
            except ActionError:
                break
            path.pop()
            return True
    path.clear()
    return False


def towards(handle: FrontierHandle, me: Pos, target: Pos) -> None:
    """One step that most closes the distance to `target`, where one is free."""
    dx, dy = target[0] - me[0], target[1] - me[1]
    for d, _ in sorted(STEPS.items(), key=lambda s: -(s[1][0] * dx + s[1][1] * dy)):
        with suppress(ActionError):
            handle.move(d)
            return
