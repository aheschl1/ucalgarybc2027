"""Frontier as a bot sees it. A bot's ``step`` receives a :class:`FrontierHandle`.

An observation's units, buildings, piles, and crops are flat integer rows; the functions
here decode them when a bot wants objects instead. :class:`Walker` and the goal functions
find walks through what a unit sees."""

import base64
from collections import deque
from collections.abc import Iterable
from contextlib import suppress
from typing import NamedTuple

from ucbc.games.frontier._api import (
    Building,
    Coord,
    Crop,
    Dir,
    FrontierApi,
    Item,
    Kind,
    Observation,
    Recipe,
    Store,
)
from ucbc.handle import ActionError

__all__ = [
    "HANDLE",
    "STEPS",
    "WALKABLE",
    "Building",
    "BuildingRow",
    "Coord",
    "Crop",
    "CropRow",
    "Dir",
    "FrontierHandle",
    "Item",
    "Kind",
    "Observation",
    "PileRow",
    "Recipe",
    "Store",
    "Tiles",
    "UnitRow",
    "Walker",
    "beside",
    "buildings",
    "crops",
    "nearest",
    "piles",
    "towards",
    "units",
]

WALKABLE = frozenset((0, 1, 4, 8))
"""Tile bytes a unit can walk on: grass, forest, dirt, farmland."""

STEPS = {Dir.UP: (0, -1), Dir.DOWN: (0, 1), Dir.LEFT: (-1, 0), Dir.RIGHT: (1, 0)}

KINDS = list(Kind)
BUILDINGS = list(Building)
ITEMS = list(Item)
CROPS = list(Crop)


class Tiles:
    """An observation's tiles, indexed by map coordinates."""

    def __init__(self, seen: Observation) -> None:
        self.origin = seen.origin
        self.width = seen.width
        self.height = seen.height
        self.data = base64.b64decode(seen.tiles)

    def __getitem__(self, at: tuple[int, int]) -> int | None:
        """The tile byte at map `(x, y)`, or None outside the observation."""
        x, y = at[0] - self.origin.x, at[1] - self.origin.y
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.data[y * self.width + x]
        return None


class UnitRow(NamedTuple):
    id: int
    team: int
    kind: Kind
    x: int
    y: int
    hp: int


class BuildingRow(NamedTuple):
    id: int
    team: int
    building: Building
    x: int
    y: int
    hp: int
    work_left: int


class PileRow(NamedTuple):
    x: int
    y: int
    item: Item
    amount: int
    """The count; `count` is a tuple method."""


class CropRow(NamedTuple):
    x: int
    y: int
    crop: Crop
    ripe_at: int


def units(seen: Observation) -> list[UnitRow]:
    r = seen.units
    return [
        UnitRow(r[i], r[i + 1], KINDS[r[i + 2]], *r[i + 3 : i + 6]) for i in range(0, len(r), 6)
    ]


def buildings(seen: Observation) -> list[BuildingRow]:
    r = seen.buildings
    return [
        BuildingRow(r[i], r[i + 1], BUILDINGS[r[i + 2]], *r[i + 3 : i + 7])
        for i in range(0, len(r), 7)
    ]


def piles(seen: Observation) -> list[PileRow]:
    r = seen.piles
    return [PileRow(r[i], r[i + 1], ITEMS[r[i + 2]], r[i + 3]) for i in range(0, len(r), 4)]


def crops(seen: Observation) -> list[CropRow]:
    r = seen.crops
    return [CropRow(r[i], r[i + 1], CROPS[r[i + 2]], r[i + 3]) for i in range(0, len(r), 4)]


Pos = tuple[int, int]


class Walker:
    """A walk through the view, taken a step at a time."""

    def __init__(self) -> None:
        self.path: list[Pos] = []
        """Tiles still to walk, the next last."""

    def step(self, handle: FrontierApi, me: Pos) -> bool:
        """Takes the next step; False, forgetting the walk, if there's none or it's blocked."""
        if self.path:
            for d, (dx, dy) in STEPS.items():
                if (me[0] + dx, me[1] + dy) == self.path[-1]:
                    with suppress(ActionError):
                        handle.move(d)
                        self.path.pop()
                        return True
        self.path.clear()
        return False

    def plan(self, seen: Observation, tiles: Tiles, me: Pos, goal: bytearray) -> bool:
        """Plans a shortest walk through the view, around units and buildings, to a tile
        marked in `goal` (indexed as the view's tiles); False if there's none."""
        w, h, ox, oy = tiles.width, tiles.height, tiles.origin.x, tiles.origin.y
        free = bytearray(t in WALKABLE for t in tiles.data)
        for rows, width in ((seen.units, 6), (seen.buildings, 7)):
            for i in range(0, len(rows), width):
                free[(rows[i + 4] - oy) * w + rows[i + 3] - ox] = 0
        start = (me[1] - oy) * w + me[0] - ox
        prev = {start: -1}
        queue = deque([start])
        self.path.clear()
        while queue:
            i = queue.popleft()
            if i != start and goal[i]:
                while i != start:
                    self.path.append((i % w + ox, i // w + oy))
                    i = prev[i]
                return True
            x = i % w
            for j in (
                i - w if i >= w else -1,
                i + w if i + w < w * h else -1,
                i - 1 if x else -1,
                i + 1 if x + 1 < w else -1,
            ):
                if j >= 0 and free[j] and j not in prev:
                    prev[j] = i
                    queue.append(j)
        return False

    def go(
        self, handle: FrontierApi, seen: Observation, tiles: Tiles, me: Pos, goal: bytearray
    ) -> bool:
        """Keeps on the walk, or plans one to `goal` and starts it."""
        return self.step(handle, me) or (self.plan(seen, tiles, me, goal) and self.step(handle, me))


def beside(tiles: Tiles, targets: Iterable[Pos]) -> bytearray:
    """A goal: the tiles in view next to any of `targets`."""
    w, h, ox, oy = tiles.width, tiles.height, tiles.origin.x, tiles.origin.y
    goal = bytearray(w * h)
    for tx, ty in targets:
        for dx, dy in STEPS.values():
            x, y = tx + dx - ox, ty + dy - oy
            if 0 <= x < w and 0 <= y < h:
                goal[y * w + x] = 1
    return goal


def nearest(tiles: Tiles, target: Pos) -> bytearray:
    """A goal for a `target` out of view: the walkable tiles in view closest to it."""
    w, ox, oy = tiles.width, tiles.origin.x, tiles.origin.y
    far = 1 << 30
    dist = [
        max(abs(i % w + ox - target[0]), abs(i // w + oy - target[1])) if t in WALKABLE else far
        for i, t in enumerate(tiles.data)
    ]
    best = min(dist)
    return bytearray(d <= best + 2 for d in dist)


def towards(handle: FrontierApi, me: Pos, target: Pos) -> None:
    """One free step that most closes the distance to `target`."""
    dx, dy = target[0] - me[0], target[1] - me[1]
    for d, _ in sorted(STEPS.items(), key=lambda s: -(s[1][0] * dx + s[1][1] * dy)):
        with suppress(ActionError):
            handle.move(d)
            return


class FrontierHandle(FrontierApi):
    pass


HANDLE = FrontierHandle
