"""Spends its step inside one C call; SIGSTOP does not care."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    sum(range(10**12))
