"""The Python runtime's behaviour under misuse and load, one test bot each."""

import json
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

from ucbc_engine.runner import run_match

BotPath = Callable[[str], Path]


def set0(bot: BotPath, name: str, tmp_path: Path) -> dict[str, Any]:
    replay = tmp_path / "replay.json"
    run_match(bot("first_empty"), bot(name), game="tictactoe", sets=1, replay_path=replay)
    result: dict[str, Any] = json.loads(replay.read_text())["sets"][0]
    return result


def steps_of(s: dict[str, Any], team: int) -> list[dict[str, Any]]:
    return [st for t in s["ticks"] for st in t["steps"] if st["team"] == team]


def test_missing_step_fails_to_load(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/no_step", tmp_path)
    assert s["result"]["reason"] == "forfeit"
    failure = steps_of(s, 1)[0]["failure"]
    assert failure["kind"] == "AttributeError"
    assert "must define step" in failure["message"]


def test_set_over_and_rejections_are_distinct(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    run_match(
        bot("testing/set_over"), bot("first_empty"), game="tictactoe", sets=1, replay_path=replay
    )
    s = json.loads(replay.read_text())["sets"][0]
    assert s["result"]["reason"] == "win"
    out = [st["stdout"].strip() for st in steps_of(s, 0)]
    assert out[:-1] == ["rejected"] * (len(out) - 1)
    assert out[-1] == "set over"


def test_unknown_query_raises_query_error(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/query_error", tmp_path)
    assert s["result"]["reason"] in ("win", "draw")
    assert "query error: unknown query type `nope`" in steps_of(s, 1)[0]["stdout"]


def test_threads_are_refused(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/threads", tmp_path)
    assert s["result"]["reason"] == "forfeit"
    failure = steps_of(s, 1)[0]["failure"]
    assert failure["kind"] == "RuntimeError"
    assert "thread" in failure["message"]


def test_sys_exit_forfeits(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/exits", tmp_path)
    assert s["result"]["reason"] == "forfeit"
    assert steps_of(s, 1)[0]["failure"]["kind"] == "SystemExit"


def test_odd_output_is_captured(bot: BotPath, tmp_path: Path) -> None:
    s = set0(bot, "testing/surrogate", tmp_path)
    assert s["result"]["reason"] in ("win", "draw")
    out = steps_of(s, 1)[0]["stdout"]
    assert "bad \\udcff char" in out
    assert "via __stdout__" in out


def test_a_thousand_queries_per_step_is_cheap(bot: BotPath, tmp_path: Path) -> None:
    started = time.perf_counter()
    set0(bot, "testing/chatty", tmp_path)
    assert time.perf_counter() - started < 3.0
