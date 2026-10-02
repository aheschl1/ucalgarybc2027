"""The UCBC 2027 game as a bot sees it. A bot's ``step`` receives a
:class:`Ucbc2027Handle`: the generated queries and actions plus a few conveniences."""

from enum import Enum

from ucbc.games.ucbc2027._api import (
    Coord,
    Dropped,
    DroppedDeposited,
    DroppedPlaced,
    Environment,
    EnvironmentEmpty,
    EnvironmentLab,
    EnvironmentWall,
    ItemView,
    ItemViewDino,
    ItemViewFossil,
    Spawned,
    Ucbc2027Api,
    UnitView,
    UnitViewDino,
    UnitViewLab,
)
from ucbc.handle import ActionError

__all__ = [
    "HANDLE",
    "Coord",
    "Direction",
    "Dropped",
    "DroppedDeposited",
    "DroppedPlaced",
    "Environment",
    "EnvironmentEmpty",
    "EnvironmentLab",
    "EnvironmentWall",
    "ItemView",
    "ItemViewDino",
    "ItemViewFossil",
    "Spawned",
    "Ucbc2027Handle",
    "UnitView",
    "UnitViewDino",
    "UnitViewLab",
]


class Direction(Enum):
    """A step to a neighbouring tile as (dx, dy); y grows downward."""

    N = (0, -1)
    NE = (1, -1)
    E = (1, 0)
    SE = (1, 1)
    S = (0, 1)
    SW = (-1, 1)
    W = (-1, 0)
    NW = (-1, -1)


def _sign(n: int) -> int:
    return (n > 0) - (n < 0)


class Ucbc2027Handle(Ucbc2027Api):
    def pos(self) -> Coord:
        """Where this dino stands. A lab has no position: TypeError."""
        me = self.me()
        if not isinstance(me, UnitViewDino):
            raise TypeError("only a dino has a position")
        return me.pos

    def move_dir(self, direction: Direction) -> Coord:
        """Move one tile in ``direction``."""
        pos = self.pos()
        dx, dy = direction.value
        if pos.x + dx < 0 or pos.y + dy < 0:
            raise ActionError("off the board")
        return self.move(pos.x + dx, pos.y + dy)

    def step_toward(self, x: int, y: int) -> Coord:
        """Move one tile toward (x, y). If that tile is blocked, tries the two
        neighbouring directions; if those are blocked too, raises the first refusal."""
        pos = self.pos()
        dx, dy = _sign(x - pos.x), _sign(y - pos.y)
        if (dx, dy) == (0, 0):
            return pos
        if dx and dy:
            tries = [(dx, dy), (dx, 0), (0, dy)]
        else:
            tries = [(dx, dy), (dx or 1, dy or 1), (dx or -1, dy or -1)]
        refusal: ActionError | None = None
        for tx, ty in tries:
            if pos.x + tx < 0 or pos.y + ty < 0:
                continue
            try:
                return self.move(pos.x + tx, pos.y + ty)
            except ActionError as e:
                refusal = refusal or e
        raise refusal or ActionError("off the board")


HANDLE = Ucbc2027Handle
