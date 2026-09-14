"""Places on a random empty cell, seeded by the engine so replays reproduce."""

import random

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    rng: random.Random = handle.memory.setdefault("rng", random.Random(handle.seed))
    row, col = rng.choice(handle.empty_cells())
    handle.place(row, col)
