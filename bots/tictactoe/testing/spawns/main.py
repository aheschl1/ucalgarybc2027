"""Imports subprocess; the snapshot leaves it out."""

import subprocess

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    subprocess.run(["ls"], check=False)
    handle.place(*handle.empty_cells()[0])
