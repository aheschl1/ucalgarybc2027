"""Frontier as a bot sees it. A bot's ``step`` receives a :class:`FrontierHandle`."""

import base64

from ucbc.games.frontier._api import Coord, Dir, FrontierApi, Kind, Observation, UnitView

__all__ = [
    "HANDLE",
    "WALKABLE",
    "Coord",
    "Dir",
    "FrontierHandle",
    "Kind",
    "Observation",
    "Tiles",
    "UnitView",
]

WALKABLE = frozenset((0, 1))
"""Tile bytes a unit can walk on: grass and forest."""


class Tiles:
    """An observation's tiles, indexed by map coordinates."""

    def __init__(self, seen: Observation) -> None:
        self.origin = seen.origin
        self.width = seen.width
        self.height = seen.height
        self.data = base64.b64decode(seen.tiles)

    def __getitem__(self, at: Coord) -> int | None:
        """The tile byte at `at`, or None outside the observation."""
        x, y = at.x - self.origin.x, at.y - self.origin.y
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.data[y * self.width + x]
        return None


class FrontierHandle(FrontierApi):
    pass


HANDLE = FrontierHandle
