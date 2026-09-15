from collections.abc import Callable

GAMES: list[str]
__version__: str

def lockdown() -> None: ...
def run_match(
    game: str,
    bot_dirs: list[str],
    *,
    step_ms: int,
    memory_bytes: int,
    sets: int = 3,
    seed: int = 0,
    match_id: str = "local",
    names: list[str] | None = None,
    replay_path: str | None = None,
    summary_path: str | None = None,
    echo_bot_output: bool = False,
    on_set: Callable[[str], object] | None = None,
) -> str: ...
