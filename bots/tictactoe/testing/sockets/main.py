"""Tries to open a socket; there is no network."""

import socket

from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    socket.socket()
    handle.place(*handle.empty_cells()[0])
