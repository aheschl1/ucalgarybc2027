import os
from collections.abc import Sequence
from typing import Any

class FrontierEnv:
    """Frontier sets for training, one policy unit waiting in each env. Arrays are bytes in
    native byte order, a row per env: `grid` uint8 [n, *grid_shape], `vector` float32
    [n, vector_len], `agent` int64 [n, 3] (env, episode, bot), `team` int64 [n], `reward`
    float32 [n, 3] (own, team, result) for each unit's previous turn. Since the last call:
    `gone` (env, episode, bot, own, team, result), `set_ends` (env, episode, winner or None,
    detail), `rejected` (env, episode, bot, actions rejected, actions sent). After an error
    it can only be dropped."""

    def __init__(
        self,
        opponents: Sequence[str | os.PathLike[str] | None],
        *,
        game_config: str | None = None,
        max_ticks: int = 2000,
        seed: int = 0,
        step_ms: int = 50,
        memory_bytes: int = 256 << 20,
        record_dir: str | os.PathLike[str] | None = None,
        record_every: int = 100,
    ) -> None: ...
    def reset(self) -> dict[str, Any]: ...
    def step(self, actions: bytes) -> dict[str, Any]:
        """`actions`: int64 [n, len(heads)]."""
    @property
    def num_envs(self) -> int: ...
    @property
    def heads(self) -> list[int]: ...
    @property
    def grid_shape(self) -> tuple[int, int, int]: ...
    @property
    def vector_len(self) -> int: ...
