"""Insists on the centre, then retries elsewhere: rejected moves are recoverable."""

from ucbc.game import ActionError
from ucbc.games.tictactoe import TicTacToeGame


def step(game: TicTacToeGame) -> None:
    try:
        game.place(1, 1)
    except ActionError as e:
        print(f"rejected: {e}")
        row, col = game.empty_cells()[0]
        game.place(row, col)
