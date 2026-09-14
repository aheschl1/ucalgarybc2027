"""Places on a random empty cell, seeded by the engine so replays reproduce."""

import random

from ucbc.games.tictactoe import TicTacToeGame


def step(game: TicTacToeGame) -> None:
    rng: random.Random = game.memory.setdefault("rng", random.Random(game.seed))
    row, col = rng.choice(game.empty_cells())
    game.place(row, col)
