"""Tries to start a thread; the interpreter refuses."""

import threading

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    threading.Thread(target=lambda: None).start()
    handle.place(*handle.empty_cells()[0])
