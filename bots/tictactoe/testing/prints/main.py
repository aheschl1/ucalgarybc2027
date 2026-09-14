import sys

from ucbc.games.tictactoe import TicTacToeHandle

print("loaded", sys.stdout.encoding, sys.stdout.writable())


def step(handle: TicTacToeHandle) -> None:
    print("tick", handle.tick, "as", handle.me.name)
    row, col = handle.empty_cells()[0]
    handle.place(row, col)
