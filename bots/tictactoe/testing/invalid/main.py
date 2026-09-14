"""Insists on the centre, then retries elsewhere: rejected moves are recoverable."""

from ucbc.games.tictactoe import TicTacToeHandle
from ucbc.handle import ActionError


def step(handle: TicTacToeHandle) -> None:
    try:
        handle.place(1, 1)
    except ActionError as e:
        print(f"rejected: {e}")
        row, col = handle.empty_cells()[0]
        handle.place(row, col)
