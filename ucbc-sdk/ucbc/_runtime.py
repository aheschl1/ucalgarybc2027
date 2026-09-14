"""Called by the engine to wrap its raw handle in the typed class for the running game."""

from importlib import import_module

from ucbc._engine import RawGame
from ucbc.game import Game


def make_game(name: str, raw: RawGame) -> Game:
    cls: type[Game] = import_module(f"ucbc.games.{name}").GAME
    return cls(raw)
