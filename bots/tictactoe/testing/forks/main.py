"""Tries to fork; the interpreter has no such call."""

import os

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    os.fork()
    handle.place(*handle.empty_cells()[0])
