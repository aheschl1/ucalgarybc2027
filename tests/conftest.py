from collections.abc import Callable
from pathlib import Path

import pytest
from ucbc import _engine

BOTS = Path(__file__).resolve().parents[1] / "bots" / "tictactoe"


def pytest_configure(config: pytest.Config) -> None:
    if "tictactoe" not in _engine.GAMES:
        pytest.exit(
            "the tests play tic-tac-toe, which this build leaves out: run `make test`, "
            "which builds every game",
            returncode=2,
        )


@pytest.fixture
def bot() -> Callable[[str], Path]:
    def path(name: str) -> Path:
        return BOTS / name

    return path
