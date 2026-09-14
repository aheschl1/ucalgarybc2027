"""Never finishes loading."""

from ucbc.games.tictactoe import TicTacToeHandle

while True:
    pass


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
