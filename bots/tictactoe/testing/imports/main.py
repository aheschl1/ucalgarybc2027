"""Imports a stdlib module the SDK itself never uses."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    import colorsys

    print(colorsys.rgb_to_hsv(1.0, 0.0, 0.0))
    handle.place(*handle.empty_cells()[0])
