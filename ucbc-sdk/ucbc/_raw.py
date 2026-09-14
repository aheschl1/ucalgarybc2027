"""The engine's raw handle as seen inside a bot's interpreter. The two bridge
callables reach the engine; everything else is identity."""

from collections.abc import Callable
from typing import Any

Bridge = Callable[[str], str]


class RawHandle:
    def __init__(
        self,
        query: Bridge,
        act: Bridge,
        *,
        bot_id: int,
        team: int,
        team_name: str,
        seed: int,
        game: str,
    ) -> None:
        self._query = query
        self._act = act
        self.bot_id = bot_id
        self.team = team
        self.team_name = team_name
        self.seed = seed
        self.game = game
        self.set_index = 0
        self.tick = 0
        self.memory: dict[str, Any] = {}

    def query(self, json: str) -> str:
        return self._query(json)

    def act(self, json: str) -> str:
        return self._act(json)
