"""Run matches from Python."""

import json
from pathlib import Path
from typing import Any

from ucbc import _engine

__all__ = ["default_game", "run_match"]


def default_game() -> str:
    """The game to play when none is named: the only one compiled in."""
    if len(_engine.GAMES) == 1:
        return _engine.GAMES[0]
    raise ValueError(f"choose a game: {', '.join(_engine.GAMES)}")


def run_match(
    *bot_dirs: str | Path,
    game: str | None = None,
    sets: int = 3,
    seed: int = 0,
    match_id: str = "local",
    names: list[str] | None = None,
    replay_path: str | Path | None = None,
    summary_path: str | Path | None = None,
    echo_bot_output: bool = False,
) -> dict[str, Any]:
    """Play a match between the given bot directories and return the match result."""
    result = _engine.run_match(
        game or default_game(),
        [str(d) for d in bot_dirs],
        sets=sets,
        seed=seed,
        match_id=match_id,
        names=names,
        replay_path=None if replay_path is None else str(replay_path),
        summary_path=None if summary_path is None else str(summary_path),
        echo_bot_output=echo_bot_output,
    )
    match_result: dict[str, Any] = json.loads(result)
    return match_result
