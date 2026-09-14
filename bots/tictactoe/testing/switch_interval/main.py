"""Tries to make its GIL uninterruptible; the function is gone."""

import sys

from ucbc.games.tictactoe import TicTacToeHandle

sys.setswitchinterval(1000)


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
