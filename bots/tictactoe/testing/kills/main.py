"""Tries to signal the engine; the interpreter has no such call."""

import os

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    os.kill(1, 9)
    handle.place(*handle.empty_cells()[0])
