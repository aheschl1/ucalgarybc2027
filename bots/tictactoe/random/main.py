"""Places on a random empty cell, seeded by the engine so replays reproduce."""

import random

from ucbc.games.tictactoe import TicTacToeHandle

rng: random.Random | None = None


def step(handle: TicTacToeHandle) -> None:
    global rng
    if rng is None:
        rng = random.Random(handle.seed)
    row, col = rng.choice(handle.empty_cells())
    handle.place(row, col)
