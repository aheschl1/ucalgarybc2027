"""Asks for twice the memory limit in one allocation."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    handle.memory["hog"] = bytes(2 * 2**30)
    handle.place(*handle.empty_cells()[0])
