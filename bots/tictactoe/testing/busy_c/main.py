"""Spends its step inside one C call; fuel counts it all the same."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    sum(range(10**12))
