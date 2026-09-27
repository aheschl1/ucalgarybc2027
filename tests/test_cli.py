from collections.abc import Callable
from pathlib import Path

from click.testing import CliRunner
from ucbc_engine.cli import main

BotPath = Callable[[str], Path]


def test_run_prints_sets_and_writes_files(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    result = CliRunner().invoke(
        main,
        [
            "run",
            str(bot("first_empty")),
            str(bot("random")),
            "--game",
            "tictactoe",
            "--seed",
            "3",
            "--replay",
            str(replay),
        ],
    )
    assert result.exit_code == 0, result.output
    assert "Set 1:" in result.output
    assert "Result:" in result.output
    assert replay.exists()


def test_run_reports_engine_errors(bot: BotPath, tmp_path: Path) -> None:
    result = CliRunner().invoke(
        main, ["run", str(bot("first_empty")), str(bot("random")), "--game", "chess"]
    )
    assert result.exit_code != 0
    assert "unknown game" in result.output
