"""Module globals and handle.memory are per bot, per set."""

from ucbc.games.tictactoe import TicTacToeHandle

calls: int = 0


def step(handle: TicTacToeHandle) -> None:
    global calls
    calls += 1
    handle.memory["n"] = handle.memory.get("n", 0) + 1
    print(f"team={handle.team} calls={calls} memory={handle.memory['n']}")
    row, col = handle.empty_cells()[0]
    handle.place(row, col)
