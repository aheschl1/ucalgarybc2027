"""The UCBC 2027 game as a bot sees it. A bot's ``step`` receives a
:class:`Ucbc2027Handle`: the generated queries and actions."""

from ucbc.games.ucbc2027._api import (
    Coord,
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

__all__ = [
    "HANDLE",
    "Coord",
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


class Ucbc2027Handle(Ucbc2027Api):
    """Conveniences over the generated API go here."""


HANDLE = Ucbc2027Handle
