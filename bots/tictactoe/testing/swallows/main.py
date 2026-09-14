"""Swallows everything and keeps looping, even the ``SystemExit`` that ends its set;
it is abandoned."""

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
