"""Asks for more than the memory limit in one allocation (and less than 2 GiB, the
most a 32-bit interpreter can even ask for)."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    handle.memory["hog"] = bytes(1100 * 2**20)
    handle.place(*handle.empty_cells()[0])
