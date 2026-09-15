"""Tries to fork; the process may not."""

import os

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    os.fork()
    handle.place(*handle.empty_cells()[0])
