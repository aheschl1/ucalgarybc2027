"""Swallows everything and keeps looping; it is stopped at the deadline and killed at
set end."""

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
