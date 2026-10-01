"""Runs inside each bot's wasm instance (``ucbc-wasm``). The guest's ``init`` imports this
module before the build snapshots the interpreter, so everything imported here, the whole
standard library included, is already loaded in every bot. The engine then calls
:func:`compile` once per team, :func:`load` once per bot and :func:`step` once per turn.

The bot's own directory is ``/bot``, first on ``sys.path``, so ``main.py`` imports its
siblings as usual. ``_host.call`` sends the engine one message and returns its reply."""

import compileall
import gc
import io
import pickle
import pkgutil
import random
import sys
import traceback
from collections.abc import Callable
from importlib import import_module
from importlib.machinery import PathFinder
from typing import Any

import _host  # type: ignore[import-not-found]

import ucbc.games
from ucbc.handle import Handle, Identity

# The whole standard library is imported into the snapshot. After that a bot can import
# only what the snapshot holds and its own modules: no path back to the zip, no builtin or
# frozen finder. Left out, so unavailable to bots: what cannot import under WASI, what
# would reach outside the sandbox, and CPython's test scaffolding.
STDLIB = "/lib/python314.zip"
EXCLUDED = frozenset(
    {
        # No zlib, bz2, lzma or termios in the interpreter; asyncio, email and xml are pruned.
        "_asyncio",
        "_elementtree",
        "bz2",
        "compression.bz2",
        "compression.gzip",
        "compression.lzma",
        "compression.zlib",
        "compression.zstd",
        "encodings.bz2_codec",
        "encodings.mbcs",
        "encodings.oem",
        "encodings.zlib_codec",
        "gzip",
        "importlib.metadata",
        "lzma",
        "plistlib",
        "pty",
        "tty",
        # Processes, signals, sockets, terminals, browsers.
        "_posixsubprocess",
        "_socket",
        "antigravity",
        "ftplib",
        "getpass",
        "logging.config",
        "logging.handlers",
        "poplib",
        "select",
        "selectors",
        "signal",
        "socket",
        "socketserver",
        "subprocess",
        "webbrowser",
        # Test scaffolding.
        "_testbuffer",
        "_testcapi",
        "_testclinic",
        "_testclinic_limited",
        "_testinternalcapi",
        "_testlimitedcapi",
        "_xxtestfuzz",
        "xxsubtype",
    }
)
for _name in sorted(
    {m.name for m in pkgutil.walk_packages([STDLIB])} | set(sys.builtin_module_names)
):
    if _name not in EXCLUDED:
        import_module(_name)

HANDLES: dict[str, type[Handle]] = {
    m.name: import_module(f"ucbc.games.{m.name}").HANDLE
    for m in pkgutil.iter_modules(ucbc.games.__path__)
}

sys.path[:] = ["/bot"]
sys.meta_path[:] = [PathFinder]
# Nothing loaded so far is garbage: bots' collections skip it.
gc.freeze()
# Where :func:`compile` writes the team's bytecode and its bots read it.
sys.pycache_prefix = "/cache"

_buffer = io.BytesIO()
_output = io.TextIOWrapper(_buffer, encoding="utf-8", errors="backslashreplace", write_through=True)
sys.stdout = sys.stderr = _output
sys.__stdout__ = sys.__stderr__ = _output  # type: ignore[misc]

_handle: Handle | None = None
_step: Callable[[Any], None] | None = None


def _call(kind: str, payload: Any) -> Any:
    return pickle.loads(_host.call(pickle.dumps({kind: payload}, protocol=5)))


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


def compile() -> None:
    compileall.compile_dir("/bot", quiet=2)


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
