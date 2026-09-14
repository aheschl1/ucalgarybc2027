"""Never returns. Frozen every turn; the ``SystemExit`` that ends its set is not an
``Exception``, so it does unwind."""

from ucbc.games.tictactoe import TicTacToeHandle


def spin() -> None:
    while True:
        pass


def step(handle: TicTacToeHandle) -> None:
    try:
        spin()
    except Exception:  # noqa: BLE001, S110
        pass
