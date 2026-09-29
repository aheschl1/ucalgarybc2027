"""Swallows everything and keeps looping; suspended at the budget, dropped at set end."""

from ucbc.games.tictactoe import TicTacToeHandle


def spin() -> None:
    while True:
        pass


def step(handle: TicTacToeHandle) -> None:
    while True:
        try:
            spin()
        except BaseException:  # noqa: BLE001, S110
            pass
