"""Imports a stdlib module that is not preloaded: it comes from the stdlib zip."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    import colorsys

    print(colorsys.rgb_to_hsv(1.0, 0.0, 0.0))
    handle.place(*handle.empty_cells()[0])
