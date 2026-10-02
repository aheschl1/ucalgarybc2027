"""Looks at every tile and reports where the dinos and fossils are."""

from ucbc.games.ucbc2027 import ItemViewDino, Ucbc2027Handle, UnitViewLab


def step(handle: Ucbc2027Handle) -> None:
    if not isinstance(handle.me(), UnitViewLab):
        return
    for y in range(handle.height()):
        for x in range(handle.width()):
            item = handle.item(x, y)
            if isinstance(item, ItemViewDino):
                who = "mine" if item.team == handle.team else "theirs"
                print(f"{who} level {item.level} dino at ({x}, {y})")
            elif item is not None:
                print(f"fossil at ({x}, {y})")
