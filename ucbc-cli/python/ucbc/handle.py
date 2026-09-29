"""The game-agnostic side of what a bot's ``step(handle)`` receives: a handle to the
one game the engine runs."""

from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

__all__ = [
    "ActionError",
    "BridgeFn",
    "Handle",
    "Identity",
    "QueryError",
    "SetOver",
]

BridgeFn = Callable[[str, dict[str, Any]], dict[str, Any]]
"""Sends one ``"query"`` or ``"act"`` payload to the engine and returns its reply."""


class QueryError(Exception):
    """The game refused a query."""


class ActionError(Exception):
    """The game refused an action. Nothing is forfeited; try something else."""


class SetOver(ActionError):
    """The set is already over."""


@dataclass(frozen=True)
class Identity:
    bot_id: int
    team: int
    team_name: str
    seed: int
    game: str


class Handle:
    """Wraps the engine's bridge. Each game subclasses this with typed methods."""

    def __init__(self, identity: Identity, bridge: BridgeFn) -> None:
        self._identity = identity
        self._bridge = bridge
        self.set_index = 0
        self.tick = 0
        self.memory: dict[str, Any] = {}
        """Persists across this bot's steps within a set."""

    @property
    def bot_id(self) -> int:
        return self._identity.bot_id

    @property
    def team(self) -> int:
        return self._identity.team

    @property
    def team_name(self) -> str:
        return self._identity.team_name

    @property
    def seed(self) -> int:
        """Per bot, per set; stable for a given match seed."""
        return self._identity.seed

    def _call(self, name: str, payload: dict[str, Any]) -> Any:
        reply = self._bridge(name, payload)
        if "err" in reply:
            kind, message = reply["err"]["kind"], reply["err"]["message"]
            if kind == "query":
                raise QueryError(message)
            if kind == "set_over":
                raise SetOver(message)
            raise ActionError(message)
        return reply["ok"]

    def _query(self, payload: dict[str, Any]) -> Any:
        return self._call("query", payload)

    def _act(self, payload: dict[str, Any]) -> Any:
        return self._call("act", payload)
