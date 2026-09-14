"""Holds 700 MB twice in a row, never both; only freed memory makes the second fit."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    for _ in range(2):
        block = bytes(700 * 2**20)
        handle.memory["size"] = len(block)
        del block
    handle.place(*handle.empty_cells()[0])
