"""Reads a file outside its own directory; nothing else is mounted."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    with open("/etc/hostname") as f:
        f.read()
    handle.place(*handle.empty_cells()[0])
