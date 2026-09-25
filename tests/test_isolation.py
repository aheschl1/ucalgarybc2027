import json
import os
from collections.abc import Callable
from pathlib import Path

import pytest

from ucbc_engine.runner import run_match

BotPath = Callable[[str], Path]


def test_bot_inherits_only_allowlisted_env(
    bot: BotPath, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    pythonpath = str(tmp_path) + os.pathsep + os.environ.get("PYTHONPATH", "")
    monkeypatch.setenv("PYTHONPATH", pythonpath)
    monkeypatch.setenv("UCBC_DATABASE_URL", "secret-db")
    monkeypatch.setenv("UCBC_BLOB_URL", "secret-blob")
    monkeypatch.setenv("BOT_PRIVATE_TOKEN", "secret-token")
    team = tmp_path / "team"
    team.mkdir()
    (team / "main.py").write_text(
        "import os\n"
        "def step(handle):\n"
        "    print(os.environ.get('PYTHONPATH'))\n"
        "    print(os.environ.get('UCBC_DATABASE_URL'))\n"
        "    print(os.environ.get('UCBC_BLOB_URL'))\n"
        "    print(os.environ.get('BOT_PRIVATE_TOKEN'))\n"
        "    handle.place(*handle.empty_cells()[0])\n"
    )
    replay = tmp_path / "replay.json"
    run_match(team, bot("first_empty"), sets=1, replay_path=replay)
    output = json.loads(replay.read_text())["sets"][0]["ticks"][0]["steps"][0]["stdout"]
    assert output.splitlines() == [pythonpath, "None", "None", "None"]


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
