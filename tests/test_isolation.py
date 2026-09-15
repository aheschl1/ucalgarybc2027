import json
from collections.abc import Callable
from pathlib import Path

from ucbc_engine.runner import run_match

BotPath = Callable[[str], Path]


def test_output_is_captured_per_step(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    run_match(bot("testing/prints"), bot("first_empty"), sets=1, replay_path=replay)
    ticks = json.loads(replay.read_text())["sets"][0]["ticks"]
    # Output written while main.py loads arrives with the first step.
    assert ticks[0]["steps"][0]["stdout"] == "loaded utf-8 True\ntick 0 as X\n"
    assert ticks[1]["steps"][0]["stdout"] == "tick 1 as X\n"
    assert ticks[0]["steps"][1].get("stdout", "") == ""


def test_bots_have_separate_globals_and_memory(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    run_match(bot("testing/counter"), bot("testing/counter"), sets=2, replay_path=replay)
    sets = json.loads(replay.read_text())["sets"]
    out = [st["stdout"].strip() for t in sets[0]["ticks"] for st in t["steps"]]
    # Each bot counts its own steps; neither sees the other's globals or memory.
    assert out[0] == "team=0 calls=1 memory=1"
    assert out[1] == "team=1 calls=1 memory=1"
    assert out[2] == "team=0 calls=2 memory=2"
    # A new set starts fresh.
    first = sets[1]["ticks"][0]["steps"][0]["stdout"].strip()
    assert first.endswith("calls=1 memory=1")
