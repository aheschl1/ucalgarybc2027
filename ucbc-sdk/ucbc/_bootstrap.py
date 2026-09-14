"""Runs inside a bot's own interpreter. The engine calls ``load`` once and
``run_step`` per step; both return JSON so the Rust side needs no object access."""

import io
import json
import sys
import traceback
from collections.abc import Callable
from importlib import import_module
from typing import Any

from ucbc._raw import RawHandle
from ucbc.handle import Handle

_buffer = io.BytesIO()
_output = io.TextIOWrapper(_buffer, encoding="utf-8", write_through=True)
_raw: RawHandle | None = None
_handle: Handle | None = None
_step: Callable[[Any], None] | None = None


def make_handle(game: str, raw: RawHandle) -> Handle:
    """The typed handle for `game`, from `ucbc.games.<game>`."""
    cls: type[Handle] = import_module(f"ucbc.games.{game}").HANDLE
    return cls(raw)


def _describe(e: BaseException) -> dict[str, str]:
    return {
        "kind": type(e).__qualname__,
        "message": str(e),
        "traceback": "".join(traceback.format_exception(e)),
    }


def load(
    source: str,
    path: str,
    identity: str,
    query: Callable[[str], str],
    act: Callable[[str], str],
) -> str:
    """Executes the team's code into a fresh namespace and builds the bot's handle."""
    global _raw, _handle, _step
    sys.stdout = sys.stderr = _output
    try:
        fields = json.loads(identity)
        _raw = RawHandle(query, act, **fields)
        _handle = make_handle(fields["game"], _raw)
        namespace: dict[str, Any] = {
            "__name__": f"ucbc_bot_{fields['bot_id']}",
            "__file__": path,
            "__builtins__": __builtins__,
        }
        exec(compile(source, path, "exec"), namespace)  # noqa: S102
        step = namespace.get("step")
        if not callable(step):
            raise AttributeError("main.py must define step(handle)")  # noqa: TRY004
        _step = step
    except BaseException as e:  # noqa: BLE001
        return json.dumps({"error": _describe(e)})
    return json.dumps({"ok": True})


def run_step(set_index: int, tick: int) -> str:
    """Runs one step; returns captured output and the failure, if any."""
    assert _raw is not None and _step is not None
    _raw.set_index = set_index
    _raw.tick = tick
    error = None
    try:
        _step(_handle)
    except BaseException as e:  # noqa: BLE001
        error = _describe(e)
    output = _buffer.getvalue().decode("utf-8", errors="replace")
    _buffer.seek(0)
    _buffer.truncate()
    return json.dumps({"stdout": output, "error": error})
