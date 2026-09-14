import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from ucbc.runner import run_match

BotPath = Callable[[str], Path]


def set0(bot: BotPath, name: str, tmp_path: Path) -> dict[str, Any]:
    replay = tmp_path / "replay.json"
    run_match(bot("first_empty"), bot(name), sets=1, replay_path=replay)
    result: dict[str, Any] = json.loads(replay.read_text())["sets"][0]
    return result


def test_exception_forfeits_with_traceback(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/raises", tmp_path)
    assert s["result"]["reason"] == "forfeit"
    assert s["result"]["winner_team"] == 0
    assert "ValueError: boom" in s["result"]["detail"]
    failure = s["ticks"][0]["steps"][1]["failure"]
    assert failure["kind"] == "ValueError"
    assert "main.py" in failure["traceback"]


def test_placing_nothing_forfeits(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/no_action", tmp_path)
    assert s["result"]["reason"] == "forfeit"
    assert "did not place" in s["result"]["detail"]


def test_syntax_error_forfeits_on_first_step(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/syntax_error", tmp_path)
    assert s["result"]["reason"] == "forfeit"
    assert s["ticks"][0]["steps"][1]["failure"]["kind"] == "SyntaxError"


def test_rejected_move_is_recoverable(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/invalid", tmp_path)
    assert s["result"]["reason"] in ("win", "draw")
    steps = [st for t in s["ticks"] for st in t["steps"] if st["team"] == 1]
    assert all(len(st["actions"]) == 1 for st in steps)
    assert any("rejected" in st.get("stdout", "") for st in steps)


def test_missing_directory_forfeits(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    run_match(bot("first_empty"), tmp_path / "nope", sets=1, replay_path=replay)
    s = json.loads(replay.read_text())["sets"][0]
    assert s["result"]["reason"] == "forfeit"
    assert s["ticks"][0]["steps"][1]["failure"]["kind"] == "FileNotFoundError"


def test_unknown_game_is_an_error(bot: BotPath) -> None:
    with pytest.raises(RuntimeError, match="unknown game"):
        run_match(bot("first_empty"), bot("random"), game="chess", sets=1)
