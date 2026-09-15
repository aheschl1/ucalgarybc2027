"""The bot process: ``python -m ucbc._bot``. Runs one team's ``main.py`` for one bot
and speaks line-delimited JSON with the engine on its real stdin and stdout. The
bot's own prints go to a buffer that is returned with each step."""

import io
import json
import os
import resource
import sys
import traceback
from collections.abc import Callable
from typing import Any, TextIO

from ucbc import _engine
from ucbc.handle import Handle, Identity

_buffer = io.BytesIO()
_output = io.TextIOWrapper(_buffer, encoding="utf-8", errors="backslashreplace", write_through=True)


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
    with open("/proc/self/statm") as f:
        return int(f.read().split()[1]) * os.sysconf("SC_PAGE_SIZE")


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
    _engine.lockdown()
    init = link.recv()
    who = Identity(**init["identity"])
    handle_cls = __import__(f"ucbc.games.{who.game}", fromlist=["HANDLE"]).HANDLE
    limit = init["memory_bytes"]
    resource.setrlimit(resource.RLIMIT_AS, (limit, limit))
    handle: Handle = handle_cls(who, link.bridge)
    try:
        step = _load(init["source"], init["path"], who.bot_id)
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
