"""Reads the state and does nothing."""

from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    print(handle.width(), handle.height())
    
