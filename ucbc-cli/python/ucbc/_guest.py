"""Runs inside each bot's wasm instance (``ucbc-wasm``). The guest's ``init`` imports this
module before the build snapshots the interpreter, so everything imported here is already
loaded in every bot. The engine then calls :func:`load` once and :func:`step` once per turn.

The bot's own directory is ``/bot``, first on ``sys.path``, so ``main.py`` imports its
siblings as usual. ``_host.call`` sends the engine one message and returns its reply."""

import io
import json
import pkgutil
import random
import sys
import traceback
from collections.abc import Callable
from importlib import import_module
from typing import Any

import _host  # type: ignore[import-not-found]

import ucbc.games
from ucbc.handle import Handle, Identity

# Loaded into the snapshot; any other stdlib module imports at the bot's own cost.
PRELOAD = (
    "abc",
    "array",
    "bisect",
    "cmath",
    "collections",
    "collections.abc",
    "contextlib",
    "copy",
    "dataclasses",
    "decimal",
    "enum",
    "fractions",
    "functools",
    "heapq",
    "itertools",
    "math",
    "numbers",
    "operator",
    "pprint",
    "random",
    "re",
    "statistics",
    "string",
    "textwrap",
    "threading",
    "time",
    "types",
    "typing",
    "weakref",
)
for _name in PRELOAD:
    import_module(_name)

HANDLES: dict[str, type[Handle]] = {
    m.name: import_module(f"ucbc.games.{m.name}").HANDLE
    for m in pkgutil.iter_modules(ucbc.games.__path__)
}

sys.path[:] = ["/bot", "/lib/python314.zip"]

_buffer = io.BytesIO()
_output = io.TextIOWrapper(_buffer, encoding="utf-8", errors="backslashreplace", write_through=True)
sys.stdout = sys.stderr = _output
sys.__stdout__ = sys.__stderr__ = _output  # type: ignore[misc]

_handle: Handle | None = None
_step: Callable[[Any], None] | None = None


def _call(kind: str, payload: Any) -> Any:
    return json.loads(_host.call(json.dumps({kind: payload}).encode()))


def _describe(e: BaseException) -> dict[str, str]:
    # Whatever filled the memory budget is still alive here, so formatting may fail too.
    try:
        formatted = "".join(traceback.format_exception(e))
    except MemoryError:
        formatted = ""
    return {"kind": type(e).__qualname__, "message": str(e), "traceback": formatted}


def _take_output() -> str:
    output = _buffer.getvalue().decode("utf-8", errors="replace")
    _buffer.seek(0)
    _buffer.truncate()
    return output


def load() -> None:
    """Imports the team's ``main.py`` and reports whether it defines ``step``."""
    global _handle, _step
    who = Identity(**_call("ready", None)["identity"])
    random.seed(who.seed)
    _handle = HANDLES[who.game](who, _call)
    try:
        step = getattr(import_module("main"), "step", None)
        if step is None:
            raise AttributeError("main.py must define step(handle)")
        if not callable(step):
            raise TypeError("step must be callable")
    except BaseException as e:  # noqa: BLE001
        _call("loaded", _describe(e))
        return
    _step = step
    _call("loaded", None)


def step(set_index: int, tick: int) -> None:
    assert _handle is not None and _step is not None, "load comes first"
    _handle.set_index, _handle.tick = set_index, tick
    error = None
    try:
        _step(_handle)
    except BaseException as e:  # noqa: BLE001
        error = _describe(e)
    _call("done", {"stdout": _take_output(), "error": error})
