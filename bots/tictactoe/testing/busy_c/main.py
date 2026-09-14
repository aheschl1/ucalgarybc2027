"""Spends its step inside one C call, which no freeze can interrupt; it is abandoned
at the end of its set."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    sum(range(10**12))
