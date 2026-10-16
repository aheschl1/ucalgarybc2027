"""Frontier as a bot sees it. A bot's ``step`` receives a :class:`FrontierHandle`.

An observation's units, buildings, piles, and crops are flat integer rows; the functions
here decode them when a bot wants objects instead."""

import base64
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
)

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
    "Tiles",
    "UnitRow",
    "buildings",
    "crops",
    "piles",
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


class FrontierHandle(FrontierApi):
    pass


HANDLE = FrontierHandle
