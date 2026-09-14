"""Module globals and game.memory are per bot, per set."""

from ucbc.games.tictactoe import TicTacToeGame

calls: int = 0


def step(game: TicTacToeGame) -> None:
    global calls
    calls += 1
    game.memory["n"] = game.memory.get("n", 0) + 1
    print(f"team={game.team} calls={calls} memory={game.memory['n']}")
    row, col = game.empty_cells()[0]
    game.place(row, col)
