"""Tries to write next to its own source; the bot's directory is read-only."""

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    with open("/bot/notes.txt", "w") as f:
        f.write("x")
    handle.place(*handle.empty_cells()[0])
