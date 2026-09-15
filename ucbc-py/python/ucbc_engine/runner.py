"""Run matches from Python."""

import json
import logging
from collections.abc import Callable
from functools import partial
from pathlib import Path
from typing import Any

from ucbc_engine import _engine
from ucbc_engine.upload import ENV_VARS, Uploader, UploadError

__all__ = ["DEFAULT_MEMORY_BYTES", "DEFAULT_STEP_MS", "default_game", "run_match"]

DEFAULT_STEP_MS = 500
DEFAULT_MEMORY_BYTES = 2**30

log = logging.getLogger(__name__)


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
    step_ms: int = DEFAULT_STEP_MS,
    memory_bytes: int = DEFAULT_MEMORY_BYTES,
    upload: bool = False,
) -> dict[str, Any]:
    """Play a match between the given bot directories and return the match result.
    ``step_ms`` and ``memory_bytes`` are each bot's budget. With ``upload`` the match is
    recorded on the API named by the UCBC_API_* variables, and skipped with a warning when
    they are missing; the API's id becomes the match id."""
    game = game or default_game()
    names = names or [Path(d).name for d in bot_dirs]
    uploader = Uploader.from_env() if upload else None
    if upload and uploader is None:
        log.warning("upload requested but %s are not all set; not uploading", ", ".join(ENV_VARS))

    upload_id: str | None = None
    on_set: Callable[[str], None] | None = None
    if uploader is not None:
        config = {"sets": sets, "seed": seed, "step_ms": step_ms, "memory_bytes": memory_bytes}
        try:
            upload_id = uploader.create_match(game, _engine.__version__, names, config)
        except UploadError as e:
            log.warning("could not create the match on %s: %s; not uploading", uploader.url, e)
        else:
            match_id = upload_id
            on_set = partial(uploader.add_set, upload_id)

    result = _engine.run_match(
        game,
        [str(d) for d in bot_dirs],
        sets=sets,
        seed=seed,
        match_id=match_id,
        names=names,
        replay_path=None if replay_path is None else str(replay_path),
        summary_path=None if summary_path is None else str(summary_path),
        echo_bot_output=echo_bot_output,
        step_ms=step_ms,
        memory_bytes=memory_bytes,
        on_set=on_set,
    )
    match_result: dict[str, Any] = json.loads(result)
    if uploader is not None and upload_id is not None:
        uploader.complete(upload_id, match_result)
        log.info("uploaded match %s to %s", upload_id, uploader.url)
    return match_result
