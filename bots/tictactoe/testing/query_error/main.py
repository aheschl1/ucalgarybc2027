"""Sends a query the game does not know."""

from ucbc.games.tictactoe import TicTacToeHandle
from ucbc.handle import QueryError


def step(handle: TicTacToeHandle) -> None:
    try:
        handle._query({"type": "nope"})
    except QueryError as e:
        print(f"query error: {e}")
    handle.place(*handle.empty_cells()[0])
