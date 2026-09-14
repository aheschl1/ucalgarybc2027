"""Places, overruns once, then places again: the second mark lands on the next
turn, where the step resumes."""

import time

from ucbc.games.tictactoe import TicTacToeHandle

OVERRUN_S = 0.7


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
    deadline = time.monotonic() + OVERRUN_S
    while time.monotonic() < deadline:
        pass
    handle.place(*handle.empty_cells()[0])
