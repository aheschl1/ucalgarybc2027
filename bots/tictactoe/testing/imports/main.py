"""Imports something not preloaded, which needs a file; the process may not."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    import subprocess  # noqa: F401

    handle.place(*handle.empty_cells()[0])
