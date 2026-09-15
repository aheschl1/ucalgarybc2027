"""Tries to stop the engine; the process may not signal anyone."""

import os
import signal

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    os.kill(os.getppid(), signal.SIGSTOP)
    handle.place(*handle.empty_cells()[0])
