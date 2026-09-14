"""Matches run with the SDK defaults: 500 ms per step and 1 GiB per bot."""

import json
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

from ucbc.runner import run_match

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


def test_looping_load_is_frozen(bot: BotPath, tmp_path: Path) -> None:
    # The load overran, the first turn resumed it, and it still never placed.
    cut_off(set0(bot, "testing/loops_on_load", tmp_path))


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


ABANDON_PROBE = """
import json, os, resource, sys, time
from ucbc import _engine
from ucbc.runner import run_match
run_match(sys.argv[1], sys.argv[2], sets=1)
before = resource.getrusage(resource.RUSAGE_SELF).ru_utime
time.sleep(1)
after = resource.getrusage(resource.RUSAGE_SELF).ru_utime
print(json.dumps({"cpu": after - before, "abandoned": _engine.abandoned_bots()}))
sys.stdout.flush()
os._exit(0)
"""


def abandon_probe(bot: BotPath, name: str) -> dict[str, Any]:
    """Runs a match in a subprocess, since an abandoned bot lives until its process
    exits, and reports the process's CPU use while idle afterwards."""
    done = subprocess.run(
        [sys.executable, "-c", ABANDON_PROBE, str(bot("first_empty")), str(bot(name))],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert done.returncode == 0, done.stderr
    probe: dict[str, Any] = json.loads(done.stdout)
    return probe


def test_abandoned_bot_thread_stops_running(bot: BotPath) -> None:
    # A bot that swallows the set-end SystemExit is left frozen, not spinning.
    probe = abandon_probe(bot, "testing/swallows")
    assert probe["abandoned"] == 1
    assert probe["cpu"] < 0.2, probe


def test_a_bot_inside_a_c_call_is_abandoned(bot: BotPath) -> None:
    # The freeze cannot land until the call returns, and neither can the SystemExit
    # at set end. Tic-tac-toe forfeits an empty step at once, so the bot is abandoned
    # at set end; the in-step rule (a whole silent turn after a freeze that never
    # landed) needs a game that plays on after an empty step.
    assert abandon_probe(bot, "testing/busy_c")["abandoned"] == 1


def test_the_switch_interval_cannot_be_changed(bot: BotPath, tmp_path: Path) -> None:
    failure = failure_of(set0(bot, "testing/switch_interval", tmp_path))
    assert failure["kind"] == "AttributeError"
    assert "setswitchinterval" in failure["message"]


def test_unstoppable_bot_is_abandoned_and_the_process_still_exits(
    bot: BotPath, tmp_path: Path
) -> None:
    """Runs in a subprocess: the abandoned thread lives until that process exits."""
    replay = tmp_path / "replay.json"
    cmd = [sys.executable, "-c", "from ucbc.cli import main; main()", "run"]
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
