"""Matches run with the SDK defaults: 500 ms per step and 1 GiB per bot."""

import json
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

from ucbc_engine.runner import run_match

BotPath = Callable[[str], Path]
STEP_MS = 500


def set0(bot: BotPath, name: str, tmp_path: Path, sets: int = 1) -> dict[str, Any]:
    replay = tmp_path / "replay.json"
    run_match(bot("first_empty"), bot(name), sets=sets, replay_path=replay)
    result: dict[str, Any] = json.loads(replay.read_text())
    return result


def steps_of(replay: dict[str, Any], bot: int, set_index: int = 0) -> list[dict[str, Any]]:
    ticks = replay["sets"][set_index]["ticks"]
    return [step for tick in ticks for step in tick["steps"] if step["bot"] == bot]


def forfeited(replay: dict[str, Any], set_index: int = 0) -> str:
    """The forfeit detail of a set team 1 lost."""
    s = replay["sets"][set_index]
    assert s["result"]["reason"] == "forfeit"
    assert s["result"]["winner_team"] == 0
    detail: str = s["result"]["detail"]
    return detail


def failure_of(replay: dict[str, Any], set_index: int = 0) -> dict[str, Any]:
    """The failure of team 1's first step, which forfeited the set."""
    forfeited(replay, set_index)
    step = replay["sets"][set_index]["ticks"][0]["steps"][1]
    assert step["actions"] == []
    failure: dict[str, Any] = step["failure"]
    return failure


def cut_off(replay: dict[str, Any], set_index: int = 0) -> None:
    """Team 1's first step was frozen at the deadline: an ordinary empty step that
    the game, not the runtime, holds against it."""
    assert "did not place a mark" in forfeited(replay, set_index)
    step = replay["sets"][set_index]["ticks"][0]["steps"][1]
    assert step["actions"] == []
    assert "failure" not in step
    assert step["usage"]["time_us"] >= STEP_MS * 1000


def test_looping_step_is_frozen_and_the_turn_is_lost(bot: BotPath, tmp_path: Path) -> None:
    started = time.monotonic()
    replay = set0(bot, "testing/loops", tmp_path)
    elapsed = time.monotonic() - started
    cut_off(replay)
    assert elapsed < 3


def test_looping_load_fails(bot: BotPath, tmp_path: Path) -> None:
    failure = failure_of(set0(bot, "testing/loops_on_load", tmp_path))
    assert failure["kind"] == "TimeoutError"
    assert "main.py took longer" in failure["message"]


def test_a_bot_inside_a_c_call_is_stopped_too(bot: BotPath, tmp_path: Path) -> None:
    cut_off(set0(bot, "testing/busy_c", tmp_path))


def test_the_process_is_locked_down(bot: BotPath, tmp_path: Path) -> None:
    for name in ("testing/kills", "testing/forks", "testing/reads", "testing/imports"):
        assert failure_of(set0(bot, name, tmp_path))["kind"] == "PermissionError", name


def test_sockets_are_refused() -> None:
    probe = "import socket; from ucbc_engine import _engine; _engine.lockdown(); socket.socket()"
    done = subprocess.run(
        [sys.executable, "-c", probe], capture_output=True, text=True, check=False
    )
    assert done.returncode != 0
    assert "PermissionError" in done.stderr


def test_an_overrun_resumes_on_the_next_turn(bot: BotPath, tmp_path: Path) -> None:
    replay = set0(bot, "testing/overrun", tmp_path)
    assert replay["sets"][0]["result"]["reason"] != "forfeit"
    first, second = steps_of(replay, bot=1)[:2]
    # Turn 1: one mark, then frozen at the deadline.
    assert len(first["actions"]) == 1
    assert "failure" not in first
    assert first["usage"]["time_us"] >= STEP_MS * 1000
    # Turn 2: the same step resumes, its second mark lands, and the turn ends.
    assert len(second["actions"]) == 1
    assert "failure" not in second
    assert second["usage"]["time_us"] < STEP_MS * 1000


def test_over_the_memory_limit_is_a_memory_error(bot: BotPath, tmp_path: Path) -> None:
    replay = set0(bot, "testing/hog", tmp_path, sets=2)
    assert failure_of(replay, 0)["kind"] == "MemoryError"
    # Set 1: the hog moves first, in a fresh interpreter with a fresh budget.
    s1 = replay["sets"][1]
    assert s1["result"]["reason"] == "forfeit"
    assert s1["ticks"][0]["steps"][0]["failure"]["kind"] == "MemoryError"


def test_freed_memory_is_uncharged(bot: BotPath, tmp_path: Path) -> None:
    for name in ("testing/frees", "testing/churn"):
        replay = set0(bot, name, tmp_path)
        assert replay["sets"][0]["result"]["reason"] == "win"


def test_steps_record_time_and_memory(bot: BotPath, tmp_path: Path) -> None:
    replay = set0(bot, "testing/churn", tmp_path)
    churn = [step["usage"] for step in steps_of(replay, bot=1)]
    assert all(0 < usage["time_us"] < STEP_MS * 1000 for usage in churn)
    # Small objects freed within the step leave their arenas mostly empty.
    assert all(2**20 < usage["memory"] < 2**26 for usage in churn)


def test_limits_are_set_per_match_and_recorded(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    run_match(bot("first_empty"), bot("testing/churn"), sets=1, replay_path=replay, step_ms=50)
    slow = json.loads(replay.read_text())
    assert slow["config"]["limits"] == {"step_ms": 50, "memory_bytes": 2**30}
    assert "did not place a mark" in forfeited(slow)
    run_match(
        bot("first_empty"), bot("testing/churn"), sets=1, replay_path=replay, memory_bytes=2**26
    )
    small = json.loads(replay.read_text())
    assert small["config"]["limits"] == {"step_ms": STEP_MS, "memory_bytes": 2**26}
    assert failure_of(small)["kind"] == "MemoryError"


def test_a_bot_that_swallows_everything_is_killed_at_set_end(bot: BotPath, tmp_path: Path) -> None:
    """Runs the CLI in a subprocess: a surviving bot process would hold its stderr
    open and the run would not return."""
    replay = tmp_path / "replay.json"
    cmd = [sys.executable, "-c", "from ucbc_engine.cli import main; main()", "run"]
    args = [str(bot("first_empty")), str(bot("testing/swallows")), "--sets", "1"]
    done = subprocess.run(
        [*cmd, *args, "--replay", str(replay)],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert done.returncode == 0, done.stderr
    assert "did not place a mark" in done.stdout
    assert "did not place a mark" in json.loads(replay.read_text())["sets"][0]["result"]["detail"]
