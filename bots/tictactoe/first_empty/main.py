"""Places on the first empty cell, row-major."""

from ucbc.games.tictactoe import TicTacToeGame


def step(game: TicTacToeGame) -> None:
    row, col = game.empty_cells()[0]
    game.place(row, col)
