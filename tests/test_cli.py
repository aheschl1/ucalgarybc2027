import base64
import gzip
import json
from collections.abc import Callable
from pathlib import Path

import pytest
from click.testing import CliRunner
from ucbc.cli import main

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


def test_run_gzips_a_replay_named_gz(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json.gz"
    args = ["run", str(bot("first_empty")), str(bot("random")), "--game", "tictactoe"]
    result = CliRunner().invoke(main, [*args, "--sets", "1", "--replay", str(replay)])
    assert result.exit_code == 0, result.output
    assert json.loads(gzip.decompress(replay.read_bytes()))["config"]["game"] == "tictactoe"


def test_run_reports_engine_errors(bot: BotPath, tmp_path: Path) -> None:
    result = CliRunner().invoke(
        main, ["run", str(bot("first_empty")), str(bot("random")), "--game", "chess"]
    )
    assert result.exit_code != 0
    assert "unknown game" in result.output


UCBC2027 = Path(__file__).resolve().parents[1] / "games" / "ucbc2027"


def test_run_plays_on_a_map_file(tmp_path: Path) -> None:
    noop = str(UCBC2027.parents[1] / "bots" / "ucbc2027" / "noop")
    standard = UCBC2027 / "maps" / "standard.map"
    replay = tmp_path / "replay.json"
    args = ["run", noop, noop, "--game", "ucbc2027", "--sets", "1", "--replay", str(replay)]
    result = CliRunner().invoke(main, [*args, "--map", str(standard)])
    assert result.exit_code == 0, result.output
    recorded = json.loads(replay.read_text())["config"]["game_config"]["map"]
    assert base64.b64decode(recorded) == standard.read_bytes()

    broken = tmp_path / "broken.map"
    broken.write_bytes(b"\xff\xff")
    result = CliRunner().invoke(main, [*args, "--map", str(broken)])
    assert result.exit_code != 0
    assert "game_config.map: not a map file" in result.output


def test_run_prints_progress_unless_no_verbose(
    bot: BotPath, capfd: pytest.CaptureFixture[str]
) -> None:
    args = ["run", str(bot("first_empty")), str(bot("random")), "--game", "tictactoe"]
    result = CliRunner().invoke(main, [*args, "--sets", "1"])
    assert result.exit_code == 0, result.output
    assert "set 1/1: done tick 1/" in capfd.readouterr().err

    result = CliRunner().invoke(main, [*args, "--sets", "1", "--no-verbose"])
    assert result.exit_code == 0, result.output
    assert "done tick" not in capfd.readouterr().err
