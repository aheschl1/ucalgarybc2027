from collections.abc import Callable
from pathlib import Path

import pytest

BOTS = Path(__file__).resolve().parents[2] / "bots" / "tictactoe"


@pytest.fixture
def bot() -> Callable[[str], Path]:
    def path(name: str) -> Path:
        return BOTS / name

    return path
