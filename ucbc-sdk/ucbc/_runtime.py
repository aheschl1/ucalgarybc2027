"""Called by the engine to wrap its raw handle in the typed handle for the running game."""

from importlib import import_module

from ucbc._engine import RawHandle
from ucbc.handle import Handle


def make_handle(name: str, raw: RawHandle) -> Handle:
    cls: type[Handle] = import_module(f"ucbc.games.{name}").HANDLE
    return cls(raw)
