"""A thousand queries per step."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    for _ in range(1000):
        handle.board()
    handle.place(*handle.empty_cells()[0])
