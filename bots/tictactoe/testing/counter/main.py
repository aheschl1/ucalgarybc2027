"""Module globals are per bot, per set."""

from ucbc.games.tictactoe import TicTacToeHandle

calls: int = 0


def step(handle: TicTacToeHandle) -> None:
    global calls
    calls += 1
    print(f"team={handle.team} calls={calls}")
    row, col = handle.empty_cells()[0]
    handle.place(row, col)
