"""Runs inside a bot's own interpreter. The engine calls ``load`` once and
``run_step`` per step; both return JSON so the Rust side needs no object access."""

import io
import json
import sys
import traceback
from collections.abc import Callable
from dataclasses import dataclass
from importlib import import_module
from typing import Any

from ucbc.handle import BridgeFn, Handle, Identity

_buffer = io.BytesIO()
_output = io.TextIOWrapper(_buffer, encoding="utf-8", errors="backslashreplace", write_through=True)


@dataclass
class _Bot:
    step: Callable[[Any], None]
    handle: Handle


_bot: _Bot | None = None


def make_handle(game: str, identity: Identity, bridge: dict[str, BridgeFn]) -> Handle:
    """The typed handle for `game`, from `ucbc.games.<game>`."""
    cls: type[Handle] = import_module(f"ucbc.games.{game}").HANDLE
    return cls(identity, bridge)


def _describe(e: BaseException) -> dict[str, str]:
    # Whatever filled the memory budget is still alive here, so formatting may fail too.
    try:
        formatted = "".join(traceback.format_exception(e))
    except MemoryError:
        formatted = ""
    return {"kind": type(e).__qualname__, "message": str(e), "traceback": formatted}


def load(source: str, path: str, identity: str, bridge: dict[str, BridgeFn]) -> str:
    """Executes the team's code into a fresh namespace and builds the bot's handle.
    Returns the failure as JSON, or ``null``."""
    global _bot
    # The runtime freezes a bot by waiting one switch interval for its GIL.
    del sys.setswitchinterval
    sys.stdout = sys.stderr = _output
    sys.__stdout__ = sys.__stderr__ = _output  # type: ignore[misc]
    try:
        who = Identity(**json.loads(identity))
        handle = make_handle(who.game, who, bridge)
        namespace: dict[str, Any] = {"__name__": f"ucbc_bot_{who.bot_id}", "__file__": path}
        exec(compile(source, path, "exec"), namespace)  # noqa: S102
        step = namespace.get("step")
        if step is None:
            raise AttributeError("main.py must define step(handle)")
        if not callable(step):
            raise TypeError("step must be callable")
        _bot = _Bot(step, handle)
    except BaseException as e:  # noqa: BLE001
        return json.dumps(_describe(e))
    return "null"


def run_step(set_index: int, tick: int) -> str:
    """Runs one step; returns captured output and the failure, if any."""
    assert _bot is not None
    _bot.handle.set_index = set_index
    _bot.handle.tick = tick
    error = None
    try:
        _bot.step(_bot.handle)
    except BaseException as e:  # noqa: BLE001
        error = _describe(e)
    output = _buffer.getvalue().decode("utf-8", errors="replace")
    _buffer.seek(0)
    _buffer.truncate()
    return json.dumps({"stdout": output, "error": error})
