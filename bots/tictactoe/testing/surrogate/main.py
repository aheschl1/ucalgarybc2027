"""Prints a lone surrogate, and to the underlying stdout: both are captured."""

import sys

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    print("bad \udcff char")
    print("via __stdout__", file=sys.__stdout__)
    handle.place(*handle.empty_cells()[0])
