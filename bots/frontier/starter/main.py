"""A starting point. The commander spawns workers while the base has spare food. Every
third worker farms wheat by the pond north of the base; the rest cut wood. Workers head
home when full or about to go hungry, unload, and take food. Each unit runs its own copy
of this file, so the module globals belong to one unit."""

import random
from collections.abc import Callable
from contextlib import suppress

from ucbc.games.frontier import (
    STEPS,
    Coord,
    Dir,
    FrontierHandle,
    Item,
    Kind,
    Observation,
    Store,
    Tiles,
    Walker,
    beside,
    crops,
    towards,
)
from ucbc.handle import ActionError

CARRY = 20
GRASS, FOREST, DIRT, FARMLAND = 0, 1, 4, 8
FOOD = (Item.WHEAT, Item.APPLE)
PACKED = 3
SEEDS = 4

Pos = tuple[int, int]

rng = random.Random()
walker = Walker()
mode = ""
wander = 0
seedless_until = 0


def step(handle: FrontierHandle) -> None:
    global mode
    seen = handle.observe()
    if seen.base is None:
        return
    base = (seen.base.x, seen.base.y)
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
    now = "home" if hungry or full or needs_seeds else "farm" if farmer else "chop"
    if now != mode:
        walker.path.clear()
        mode = now
    if mode == "home":
        home(handle, seen, tiles, me, base, carried, {Item.WHEAT_SEED: SEEDS if farmer else 0})
    elif mode == "farm":
        farm(handle, seen, tiles, me, base, carried)
    else:
        chop(handle, seen, tiles, me)


def commander(handle: FrontierHandle, seen: Observation, food: int) -> None:
    store = base_store(seen)
    if store is None:
        return
    stored = {s.item: s.count for s in store.stored}
    with suppress(ActionError):
        if food < PACKED:
            take_food(handle, toward(seen, store.pos), stored, PACKED - food)
        elif stored.get(Item.WHEAT, 0) >= 15:
            handle.spawn()


def base_store(seen: Observation) -> Store | None:
    """The base, if it's beside you: `stores` also lists your team's sites."""
    return next((s for s in seen.stores if s.pos == seen.base), None)


def toward(seen: Observation, to: Coord) -> Dir:
    me = seen.me.pos
    return next(d for d, (dx, dy) in STEPS.items() if (me.x + dx, me.y + dy) == (to.x, to.y))


def take_food(handle: FrontierHandle, d: Dir, stored: dict[Item, int], n: int) -> bool:
    for item in FOOD:
        if stored.get(item):
            with suppress(ActionError):
                handle.take(d, item, n)
                return True
    return False


def walk(
    handle: FrontierHandle, seen: Observation, tiles: Tiles, me: Pos, goal: Callable[[], bytearray]
) -> bool:
    """Keeps on the walk, or plans one to `goal()` and starts it."""
    return walker.step(handle, me) or (
        walker.plan(seen, tiles, me, goal()) and walker.step(handle, me)
    )


def home(
    handle: FrontierHandle,
    seen: Observation,
    tiles: Tiles,
    me: Pos,
    base: Pos,
    carried: dict[Item, int],
    keep: dict[Item, int],
    food: int = PACKED,
) -> None:
    """Beside the base, unload one kind of item, or take food and what to `keep`; else
    head there."""
    global seedless_until
    store = base_store(seen)
    if store is None:
        if not walk(handle, seen, tiles, me, lambda: beside(tiles, [base])):
            towards(handle, me, base)
        return
    d = toward(seen, store.pos)
    stored = {s.item: s.count for s in store.stored}
    keep = {**keep, Item.WHEAT: food}
    for item, n in carried.items():
        extra = n - keep.get(item, 0)
        # Into the base, or beside it when the base is full.
        for to in [d, *(s for s in STEPS if s != d)] if extra > 0 else []:
            with suppress(ActionError):
                handle.put(to, item, extra)
                return
    eaten = sum(carried.get(f, 0) for f in FOOD)
    if eaten < food and take_food(handle, d, stored, food - eaten):
        return
    for item, want in keep.items():
        if item not in FOOD and carried.get(item, 0) < want and stored.get(item):
            with suppress(ActionError):
                handle.take(d, item, want - carried.get(item, 0))
                return
    if keep.get(Item.WHEAT_SEED) and not stored.get(Item.WHEAT_SEED):
        seedless_until = seen.tick + 60


def farm(
    handle: FrontierHandle,
    seen: Observation,
    tiles: Tiles,
    me: Pos,
    base: Pos,
    carried: dict[Item, int],
) -> None:
    """Harvest what's ripe, plant empty farmland, till the rest of the field."""
    bx, by = base
    field = [(x, y) for y in range(by - 10, by - 1) for x in range(bx - 4, bx + 5)]
    growing = {(c.x, c.y): c.ripe_at for c in crops(seen)}
    ripe = [at for at in field if at in growing and growing[at] <= seen.tick]
    empty = (
        [at for at in field if tiles[at] == FARMLAND and at not in growing]
        if carried.get(Item.WHEAT_SEED)
        else []
    )
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
                    walker.path.clear()
                    return
    walk(handle, seen, tiles, me, lambda: beside(tiles, ripe or empty or wild))


def chop(handle: FrontierHandle, seen: Observation, tiles: Tiles, me: Pos) -> None:
    """Gather from forest beside, walk to forest in view, or wander a while."""
    global wander
    for d, (dx, dy) in STEPS.items():
        if tiles[(me[0] + dx, me[1] + dy)] == FOREST:
            with suppress(ActionError):
                handle.gather(d)
                walker.path.clear()
                return
    if walker.step(handle, me):
        return
    if wander:
        wander -= 1
    else:
        w, ox, oy = tiles.width, tiles.origin.x, tiles.origin.y
        forest = [(i % w + ox, i // w + oy) for i, t in enumerate(tiles.data) if t == FOREST]
        if walker.plan(seen, tiles, me, beside(tiles, forest)) and walker.step(handle, me):
            return
        wander = 10
    towards(handle, me, (me[0] + rng.randint(-9, 9), me[1] + rng.randint(-9, 9)))
