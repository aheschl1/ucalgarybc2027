"""Plays one claimed match with `ucbc run` and reads back its replay."""

import asyncio
import io
import logging
import shlex
import sys
import tempfile
import time
import zipfile
from asyncio.subprocess import DEVNULL, PIPE
from collections import deque
from collections.abc import Awaitable, Callable
from pathlib import Path
from uuid import UUID

from pydantic import ValidationError

from api.blobs import BlobStore
from api.models.matches import MatchReplay, MatchRow
from api.services.submissions import check_member_name, key_for

log = logging.getLogger(__name__)

# The `ucbc` script installed next to this interpreter.
UCBC = Path(sys.executable).with_name("ucbc")
STDERR_TAIL = 800
# Engine stderr lines held for the failure message; every line is logged as it arrives.
STDERR_LINES = 50


class MatchFailed(Exception):
    """The match could not be played; it is recorded as an error."""


class StartFailed(Exception):
    """The engine process could not start; the match goes back to the queue."""


async def fetch(submission: UUID, scratch: Path, blobs: BlobStore) -> Path:
    """The directory holding the bot's main.py: the submission's zip downloaded and unpacked
    into `scratch`."""
    data = await blobs.get(key_for(submission))
    log.debug("submission %s: %d bytes into %s", submission, len(data), scratch)
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
        fetching = time.monotonic()
        bots = [await fetch(s, scratch / f"bot{i}", blobs) for i, s in enumerate(match.bots)]
        log.debug(
            "match %s: bots ready in %.2fs: %s",
            match.id,
            time.monotonic() - fetching,
            ", ".join(str(b) for b in bots),
        )
        replay = scratch / "replay.json"
        argv = command(match, bots, replay)
        log.debug("match %s: %s", match.id, shlex.join(argv))
        started = time.monotonic()
        try:
            proc = await asyncio.create_subprocess_exec(*argv, stdout=DEVNULL, stderr=PIPE)
        except OSError as e:
            raise StartFailed(str(e)) from e
        log.info(
            "match %s: engine pid %d, %s, %d sets, seed %d, %d ms and %d MiB per step",
            match.id,
            proc.pid,
            match.game,
            match.config.sets,
            match.config.seed,
            match.config.step_ms,
            match.config.memory_bytes >> 20,
        )

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
                log.debug("match %s: heartbeat", match.id)

        assert proc.stderr is not None
        tail: deque[str] = deque(maxlen=STDERR_LINES)
        draining = asyncio.create_task(_drain(proc.stderr, match.id, tail))
        beating = asyncio.create_task(beat())
        try:
            await proc.wait()
            await draining
        finally:
            beating.cancel()
            draining.cancel()
            await asyncio.gather(beating, draining, return_exceptions=True)
        elapsed = time.monotonic() - started

        if proc.returncode != 0:
            raise MatchFailed(
                f"ucbc run exited {proc.returncode} after {elapsed:.1f}s: {_tail(tail)}"
            )
        try:
            data = replay.read_bytes()
            parsed = MatchReplay.model_validate_json(data)
        except (OSError, ValidationError) as e:
            raise MatchFailed(f"unreadable replay: {e}") from e
        log.info(
            "match %s: engine finished in %.1fs, %d sets, %d byte replay",
            match.id,
            elapsed,
            len(parsed.sets),
            len(data),
        )
        return parsed


async def _drain(stream: asyncio.StreamReader, match_id: UUID, tail: deque[str]) -> None:
    """Logs the engine's stderr as it arrives and keeps the last lines for the failure
    message, so a long-running match is visible and a noisy one is not held in memory."""
    while True:
        try:
            raw = await stream.readline()
        except ValueError:
            log.warning("match %s: dropped an over-long engine stderr line", match_id)
            continue
        line = raw.decode(errors="replace").rstrip()
        if not line:
            if stream.at_eof():
                return
            continue
        tail.append(line)
        log.debug("match %s: engine: %s", match_id, line)


def _tail(lines: deque[str]) -> str:
    return "\n".join(lines)[-STDERR_TAIL:].strip()
