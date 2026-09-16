"""Plays one claimed match with `ucbc run` and reads back its replay."""

import asyncio
import io
import logging
import sys
import tempfile
import zipfile
from asyncio.subprocess import DEVNULL, PIPE
from collections.abc import Awaitable, Callable
from pathlib import Path

from pydantic import ValidationError

from api.blobs import BlobStore
from api.models.matches import BotSource, MatchReplay, MatchRow, PathSource
from api.services.submissions import check_member_name, key_for

log = logging.getLogger(__name__)

# The `ucbc` script installed next to this interpreter.
UCBC = Path(sys.executable).with_name("ucbc")
STDERR_TAIL = 800


class MatchFailed(Exception):
    """The match could not be played; it is recorded as an error."""


class StartFailed(Exception):
    """The engine process could not start; the match goes back to the queue."""


async def fetch(source: BotSource, scratch: Path, blobs: BlobStore) -> Path:
    """The directory holding the bot's main.py: a path as given, or a submission's zip
    downloaded and unpacked into `scratch`."""
    if isinstance(source, PathSource):
        return Path(source.path)
    data = await blobs.get(key_for(source.id))
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        for name in zf.namelist():
            check_member_name(name)
        zf.extractall(scratch)
    return scratch


def command(match: MatchRow, bots: list[Path], replay: Path) -> list[str]:
    """The engine process, described in one place."""
    c = match.config
    return [
        str(UCBC),
        "run",
        str(bots[0]),
        str(bots[1]),
        "--game",
        match.game,
        "--sets",
        str(c.sets),
        "--seed",
        str(c.seed),
        "--match-id",
        str(match.id),
        "--step-ms",
        str(c.step_ms),
        "--memory-mb",
        str(c.memory_bytes >> 20),
        "--replay",
        str(replay),
    ]


async def play(
    match: MatchRow,
    blobs: BlobStore,
    keepalive: Callable[[], Awaitable[bool]],
    interval: float,
) -> MatchReplay:
    """Runs the match. `keepalive` is called every `interval` seconds while the engine
    runs; when it returns False the engine is stopped and the match fails."""
    with tempfile.TemporaryDirectory(prefix="ucbc-match-") as tmp:
        scratch = Path(tmp)
        bots = [await fetch(s, scratch / f"bot{i}", blobs) for i, s in enumerate(match.bots)]
        replay = scratch / "replay.json"
        try:
            proc = await asyncio.create_subprocess_exec(
                *command(match, bots, replay), stdout=DEVNULL, stderr=PIPE
            )
        except OSError as e:
            raise StartFailed(str(e)) from e

        async def beat() -> None:
            while True:
                await asyncio.sleep(interval)
                try:
                    alive = await keepalive()
                except Exception:
                    log.exception("match %s: heartbeat failed", match.id)
                    continue
                if not alive:
                    log.warning("match %s: lease lost, stopping the engine", match.id)
                    proc.terminate()
                    return

        beating = asyncio.create_task(beat())
        try:
            _, stderr = await proc.communicate()
        finally:
            beating.cancel()
            await asyncio.gather(beating, return_exceptions=True)

        if proc.returncode != 0:
            tail = stderr[-STDERR_TAIL:].decode(errors="replace").strip()
            raise MatchFailed(f"ucbc run exited {proc.returncode}: {tail}")
        try:
            return MatchReplay.model_validate_json(replay.read_bytes())
        except (OSError, ValidationError) as e:
            raise MatchFailed(f"unreadable replay: {e}") from e
