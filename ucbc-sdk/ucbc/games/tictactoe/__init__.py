"""Tic-tac-toe as a bot sees it. A bot's ``step`` receives a :class:`TicTacToeHandle`:
the generated queries and actions plus a few conveniences."""

from ucbc.games.tictactoe._api import BoardView, Cell, Placed, TicTacToeApi

__all__ = ["HANDLE", "BoardView", "Cell", "Placed", "TicTacToeHandle"]


class TicTacToeHandle(TicTacToeApi):
    def cell(self, row: int, col: int) -> Cell:
        return self.board().cells[row * 3 + col]

    def empty_cells(self) -> list[tuple[int, int]]:
        return [(i // 3, i % 3) for i, c in enumerate(self.board().cells) if c == Cell.EMPTY]

    @property
    def me(self) -> Cell:
        """The mark this bot plays."""
        return self.board().you

    @property
    def turn(self) -> int:
        """Marks placed so far this set."""
        return self.board().turn


HANDLE = TicTacToeHandle
