import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
from ucbc_engine.runner import default_game, run_match

BotPath = Callable[[str], Path]


def test_random_vs_first_empty_plays_three_sets(bot: BotPath, tmp_path: Path) -> None:
    replay = tmp_path / "replay.json"
    summary = tmp_path / "summary.json"
    result = run_match(
        bot("random"),
        bot("first_empty"),
        game="tictactoe",
        seed=7,
        replay_path=replay,
        summary_path=summary,
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
    run_match(bot("random"), bot("random"), game="tictactoe", seed=42, replay_path=paths[0])
    run_match(bot("random"), bot("random"), game="tictactoe", seed=42, replay_path=paths[1])
    run_match(bot("random"), bot("random"), game="tictactoe", seed=43, replay_path=paths[2])

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

    assert "tictactoe" in _engine.GAMES
    assert _engine.GAMES == sorted(_engine.GAMES)


def test_the_default_game_is_the_only_one_compiled_in(monkeypatch: pytest.MonkeyPatch) -> None:
    from ucbc_engine import _engine

    monkeypatch.setattr(_engine, "GAMES", ["tictactoe"])
    assert default_game() == "tictactoe"
    monkeypatch.setattr(_engine, "GAMES", ["other", "tictactoe"])
    with pytest.raises(ValueError, match="choose a game: other, tictactoe"):
        default_game()


def test_ucbc2027_runs_to_the_tick_limit(tmp_path: Path) -> None:
    noop = Path(__file__).resolve().parents[1] / "bots" / "ucbc2027" / "noop"
    replay = tmp_path / "replay.json"
    result = run_match(noop, noop, game="ucbc2027", sets=1, replay_path=replay)
    assert result["sets"][0]["reason"] == "draw"
    ticks = json.loads(replay.read_text())["sets"][0]["ticks"]
    assert ticks[-1]["state_after"] == {"tick": len(ticks)}
    assert not any("failure" in s for t in ticks for s in t["steps"])


def test_names_default_to_directory_names(bot: BotPath) -> None:
    result = run_match(bot("first_empty"), bot("first_empty"), game="tictactoe", sets=1)
    assert result["winner_team"] == 0
