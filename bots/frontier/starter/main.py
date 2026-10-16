"""A starting point. The commander spawns workers whenever the base can pay; each worker
finds forest in view, gathers wood until full, carries it home, and puts it in the base.
Each unit runs its own copy of this file, so the module globals belong to one unit."""

import random
from collections import deque
from contextlib import suppress

from ucbc.games.frontier import STEPS, WALKABLE, FrontierHandle, Item, Kind, Observation, Tiles
from ucbc.handle import ActionError

CARRY = 20
FOREST = 1

Pos = tuple[int, int]

rng = random.Random()
# The walk being followed, its next tile last.
path: list[Pos] = []
# Steps to wander before looking for forest again, after a look found none.
wander = 0


def step(handle: FrontierHandle) -> None:
    global wander
    seen = handle.observe()
    if seen.me.kind == Kind.COMMANDER:
        with suppress(ActionError):
            handle.spawn()
        return
    rng.seed(seen.me.id * 7919 + seen.tick)
    tiles = Tiles(seen)
    me = (seen.me.pos.x, seen.me.pos.y)
    if sum(s.count for s in seen.me.carrying) >= CARRY:
        home(handle, seen, tiles, me)
        return
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
        w, data = tiles.width, tiles.data
        goal = bytearray(len(data))
        for i, t in enumerate(data):
            if t == FOREST:
                x = i % w
                for j in (i - w, i + w, i - 1 if x else -1, i + 1 if x + 1 < w else -1):
                    if 0 <= j < len(data):
                        goal[j] = 1
        plan(seen, tiles, me, goal)
        if follow(handle, me):
            return
        wander = 10
    towards(handle, me, (me[0] + rng.randint(-9, 9), me[1] + rng.randint(-9, 9)))


def home(handle: FrontierHandle, seen: Observation, tiles: Tiles, me: Pos) -> None:
    """Put the wood in the base from beside it, or head there."""
    base = (seen.base.x, seen.base.y)
    for d, (dx, dy) in STEPS.items():
        if (me[0] + dx, me[1] + dy) == base:
            # Refused when the base is full: wait.
            with suppress(ActionError):
                handle.put(d, Item.WOOD, CARRY)
            path.clear()
            return
    if follow(handle, me):
        return
    goal = bytearray(len(tiles.data))
    for dx, dy in STEPS.values():
        x, y = base[0] + dx - tiles.origin.x, base[1] + dy - tiles.origin.y
        if 0 <= x < tiles.width and 0 <= y < tiles.height:
            goal[y * tiles.width + x] = 1
    plan(seen, tiles, me, goal)
    if not follow(handle, me):
        towards(handle, me, base)


def plan(seen: Observation, tiles: Tiles, me: Pos, goal: bytearray) -> None:
    """A shortest walk through the view to a tile marked in `goal` (indexed as the
    view's tiles), into `path`."""
    w, h, ox, oy = tiles.width, tiles.height, tiles.origin.x, tiles.origin.y
    open_ = bytearray(t in WALKABLE for t in tiles.data)
    for rows, width in ((seen.units, 5), (seen.buildings, 7)):
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
    for d, (sx, sy) in sorted(STEPS.items(), key=lambda s: -(s[1][0] * dx + s[1][1] * dy)):
        with suppress(ActionError):
            handle.move(d)
            return
