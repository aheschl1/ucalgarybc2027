"""Tic-tac-toe as a bot sees it. A bot's ``step`` receives a :class:`TicTacToeHandle`."""

from enum import IntEnum

from ucbc.handle import Handle

__all__ = ["HANDLE", "Cell", "TicTacToeHandle"]


class Cell(IntEnum):
    EMPTY = 0
    X = 1
    O = 2


class TicTacToeHandle(Handle):
    def board(self) -> list[list[Cell]]:
        """The board as three rows of three cells."""
        cells = [Cell(c) for c in self._query({"type": "board"})["cells"]]
        return [cells[0:3], cells[3:6], cells[6:9]]

    def cell(self, row: int, col: int) -> Cell:
        return self.board()[row][col]

    def empty_cells(self) -> list[tuple[int, int]]:
        return [
            (r, c)
            for r, row in enumerate(self.board())
            for c, cell in enumerate(row)
            if cell == Cell.EMPTY
        ]

    @property
    def me(self) -> Cell:
        """The mark this bot plays."""
        return Cell(self._query({"type": "board"})["you"])

    @property
    def turn(self) -> int:
        """Marks placed so far this set."""
        return int(self._query({"type": "board"})["turn"])

    def place(self, row: int, col: int) -> None:
        """Place this bot's mark. Raises :class:`ucbc.handle.ActionError` if refused."""
        self._act({"type": "place", "row": row, "col": col})


HANDLE = TicTacToeHandle
