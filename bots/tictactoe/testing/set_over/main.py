"""Places, then places again: rejected while playing, SetOver after the winning move."""

from ucbc.games.tictactoe import TicTacToeHandle
from ucbc.handle import ActionError, SetOver


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
    try:
        handle.place(*handle.empty_cells()[0])
    except SetOver:
        print("set over")
    except ActionError:
        print("rejected")
