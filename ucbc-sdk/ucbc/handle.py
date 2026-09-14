"""The game-agnostic side of what a bot's ``step(handle)`` receives: a handle to the
one game the engine runs."""

import json
from typing import Any

from ucbc._raw import RawHandle

__all__ = ["ActionError", "Handle", "QueryError", "SetOver"]


class QueryError(Exception):
    """The game refused a query."""


class ActionError(Exception):
    """The game refused an action. Nothing is forfeited; try something else."""


class SetOver(ActionError):
    """The set is already over."""


class Handle:
    """Wraps the engine's raw handle. Each game subclasses this with typed methods."""

    def __init__(self, raw: RawHandle) -> None:
        self._raw = raw

    @property
    def bot_id(self) -> int:
        return self._raw.bot_id

    @property
    def team(self) -> int:
        return self._raw.team

    @property
    def team_name(self) -> str:
        return self._raw.team_name

    @property
    def set_index(self) -> int:
        return self._raw.set_index

    @property
    def tick(self) -> int:
        return self._raw.tick

    @property
    def seed(self) -> int:
        """Per bot, per set; stable for a given match seed."""
        return self._raw.seed

    @property
    def memory(self) -> dict[str, Any]:
        """Persists across this bot's steps within a set."""
        return self._raw.memory

    def _query(self, payload: dict[str, Any]) -> dict[str, Any]:
        reply = json.loads(self._raw.query(json.dumps(payload)))
        if "err" in reply:
            raise QueryError(reply["err"]["message"])
        response: dict[str, Any] = reply["ok"]
        return response

    def _act(self, payload: dict[str, Any]) -> dict[str, Any]:
        reply = json.loads(self._raw.act(json.dumps(payload)))
        if "err" in reply:
            err = reply["err"]
            if err["kind"] == "set_over":
                raise SetOver(err["message"])
            raise ActionError(err["message"])
        response: dict[str, Any] = reply["ok"]
        return response
