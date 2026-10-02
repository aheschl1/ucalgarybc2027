"""Allocates ~300 MB of small objects per step in two rounds and drops them; over a
set that is more than the limit in total, so it survives only if freed memory is
uncharged."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    for _ in range(2):
        junk = [bytes(470) for _ in range(300_000)]
        del junk
    handle.place(*handle.empty_cells()[0])
