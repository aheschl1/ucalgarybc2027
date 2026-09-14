import sys

from ucbc.games.tictactoe import TicTacToeGame

print("loaded", sys.stdout.encoding, sys.stdout.writable())


def step(game: TicTacToeGame) -> None:
    print("tick", game.tick, "as", game.me.name)
    row, col = game.empty_cells()[0]
    game.place(row, col)
