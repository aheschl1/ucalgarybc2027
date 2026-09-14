"""Places on the first empty cell, row-major."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    row, col = handle.empty_cells()[0]
    handle.place(row, col)
