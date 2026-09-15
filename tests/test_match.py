import json
import logging
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from ucbc_engine.runner import run_match

BotPath = Callable[[str], Path]


def test_random_vs_first_empty_plays_three_sets(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    summary = tmp_path / "summary.json"
    result = run_match(
        bot("random"), bot("first_empty"), seed=7, replay_path=replay, summary_path=summary
    )
    assert len(result["sets"]) == 3
    assert sum(result["set_wins"]) == sum(1 for s in result["sets"] if s["winner_team"] is not None)
    assert all(s["reason"] in ("win", "draw") for s in result["sets"])

    data = json.loads(replay.read_text())
    assert [t["name"] for t in data["teams"]] == ["random", "first_empty"]
    last_tick = data["sets"][0]["ticks"][-1]
    assert len(last_tick["state_after"]["cells"]) == 9
    assert json.loads(summary.read_text())["set_wins"] == result["set_wins"]


def test_same_seed_gives_identical_replays(bot: BotPath, tmp_path: Path) -> None:
    paths = [tmp_path / f"r{i}.json" for i in range(3)]
    run_match(bot("random"), bot("random"), seed=42, replay_path=paths[0])
    run_match(bot("random"), bot("random"), seed=42, replay_path=paths[1])
    run_match(bot("random"), bot("random"), seed=43, replay_path=paths[2])

    # Everything but the measured step times must repeat.
    def played(path: Path) -> list[Any]:
        replay = json.loads(path.read_text())
        return [
            (tick["state_after"], [step["actions"] for step in tick["steps"]])
            for s in replay["sets"]
            for tick in s["ticks"]
        ]

    assert played(paths[0]) == played(paths[1])
    assert played(paths[0]) != played(paths[2])


def test_games_lists_what_is_compiled_in() -> None:
    from ucbc_engine import _engine

    assert _engine.GAMES == ["tictactoe"]


def test_names_default_to_directory_names(bot: BotPath) -> None:
    result = run_match(bot("first_empty"), bot("first_empty"), sets=1)
    assert result["winner_team"] == 0


def test_upload_without_credentials_warns_and_plays(
    bot: BotPath, monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    for name in ("UCBC_API_URL", "UCBC_API_USERNAME", "UCBC_API_PASSWORD"):
        monkeypatch.delenv(name, raising=False)
    with caplog.at_level(logging.WARNING, logger="ucbc_engine.runner"):
        result = run_match(bot("first_empty"), bot("first_empty"), sets=1, upload=True)
    assert result["winner_team"] == 0
    assert "not uploading" in caplog.text
