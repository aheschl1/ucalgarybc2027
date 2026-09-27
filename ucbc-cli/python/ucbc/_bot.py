"""The bot process: ``python -m ucbc._bot``. Runs one team's ``main.py`` for one bot
and speaks line-delimited JSON with the engine on its real stdin and stdout. The
bot's own prints go to a buffer that is returned with each step.

Before the team's code runs the process is locked down (``_engine.lockdown``): from
then on it cannot open files, so a bot can import only what is preloaded here."""

import io
import json
import linecache
import os
import resource
import sys
import traceback
from collections.abc import Callable
from importlib import import_module
from typing import Any, TextIO

from ucbc import _engine
from ucbc.handle import Handle, Identity

# Standard library a bot may import. Anything else needs a file the process may not
# open once locked down. `ucbc` and its game modules are imported below.
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
    "encodings.ascii",
    "encodings.latin_1",
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

_buffer = io.BytesIO()
_output = io.TextIOWrapper(_buffer, encoding="utf-8", errors="backslashreplace", write_through=True)
_statm = os.open("/proc/self/statm", os.O_RDONLY)
_page = os.sysconf("SC_PAGE_SIZE")


class Link:
    """The engine's end of the pipe, taken before the bot can see stdin or stdout."""

    def __init__(self, inbound: TextIO, outbound: TextIO) -> None:
        self._in = inbound
        self._out = outbound

    def send(self, message: dict[str, Any]) -> None:
        self._out.write(json.dumps(message) + "\n")
        self._out.flush()

    def recv(self) -> dict[str, Any]:
        line = self._in.readline()
        if not line:  # The engine is gone.
            raise SystemExit
        message: dict[str, Any] = json.loads(line)
        return message

    def bridge(self, kind: str, payload: dict[str, Any]) -> dict[str, Any]:
        """One query or action, answered by the engine."""
        self.send({kind: payload})
        reply: dict[str, Any] = self.recv()["reply"]
        return reply


def _describe(e: BaseException) -> dict[str, str]:
    # Whatever filled the memory budget is still alive here, so formatting may fail too.
    try:
        formatted = "".join(traceback.format_exception(e))
    except MemoryError:
        formatted = ""
    return {"kind": type(e).__qualname__, "message": str(e), "traceback": formatted}


def _load(source: str, path: str, bot_id: int) -> Callable[[Any], None]:
    """Executes the team's code into a fresh namespace and returns its ``step``."""
    namespace: dict[str, Any] = {"__name__": f"ucbc_bot_{bot_id}", "__file__": path}
    exec(compile(source, path, "exec"), namespace)  # noqa: S102
    step = namespace.get("step")
    if step is None:
        raise AttributeError("main.py must define step(handle)")
    if not callable(step):
        raise TypeError("step must be callable")
    return step  # type: ignore[no-any-return]


def _resident_bytes() -> int:
    return int(os.pread(_statm, 256, 0).split()[1]) * _page


def _take_output() -> str:
    output = _buffer.getvalue().decode("utf-8", errors="replace")
    _buffer.seek(0)
    _buffer.truncate()
    return output


def main() -> None:
    link = Link(sys.stdin, sys.stdout)
    sys.stdin = io.StringIO()
    sys.stdout = sys.stderr = _output
    sys.__stdout__ = sys.__stderr__ = _output  # type: ignore[misc]
    init = link.recv()
    who = Identity(**init["identity"])
    handle_cls = import_module(f"ucbc.games.{who.game}").HANDLE
    for name in PRELOAD:
        import_module(name)
    source, path = init["source"], init["path"]
    # Tracebacks read source through linecache, which may not open the file later.
    linecache.cache[path] = (len(source), None, source.splitlines(True), path)
    limit = init["memory_bytes"]
    resource.setrlimit(resource.RLIMIT_AS, (limit, limit))
    _engine.lockdown()
    handle: Handle = handle_cls(who, link.bridge)
    link.send({"ready": None})  # The load budget starts here.
    try:
        step = _load(source, path, who.bot_id)
    except BaseException as e:  # noqa: BLE001
        link.send({"loaded": _describe(e)})
        return
    link.send({"loaded": None})
    while True:
        turn = link.recv()["step"]
        handle.set_index, handle.tick = turn["set_index"], turn["tick"]
        error = None
        try:
            step(handle)
        except BaseException as e:  # noqa: BLE001
            error = _describe(e)
        done = {"stdout": _take_output(), "error": error, "memory": _resident_bytes()}
        link.send({"done": done})


if __name__ == "__main__":
    main()
