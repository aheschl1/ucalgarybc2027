"""Tries to read a file, its own source even; the process may not open anything."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    with open(__file__) as f:
        f.read()
    handle.place(*handle.empty_cells()[0])
