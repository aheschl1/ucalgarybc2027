"""The UCBC 2027 game as a bot sees it. A bot's ``step`` receives a
:class:`Ucbc2027Handle`: the generated queries and actions."""

from ucbc.games.ucbc2027._api import State, Ucbc2027Api

__all__ = ["HANDLE", "State", "Ucbc2027Handle"]


class Ucbc2027Handle(Ucbc2027Api):
    """Conveniences over the generated API go here."""


HANDLE = Ucbc2027Handle
