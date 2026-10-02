"""Looks at every tile and reports where the players stand."""

from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    for y in range(handle.height()):
        for x in range(handle.width()):
            item = handle.item(x, y)
            if item is not None:
                who = "me" if item.team == handle.team else "them"
                print(f"{who} at ({x}, {y}) on {handle.environment(x, y).value}")
    handle.noop()
