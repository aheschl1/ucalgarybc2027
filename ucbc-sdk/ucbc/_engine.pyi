from typing import Any

class RawGame:
    bot_id: int
    team: int
    team_name: str
    set_index: int
    tick: int
    seed: int
    game: str
    memory: dict[str, Any]
    def query(self, json: str) -> str: ...
    def act(self, json: str) -> str: ...

class QueryError(Exception): ...
class ActionError(Exception): ...
class SetOver(ActionError): ...

GAMES: list[str]
__version__: str

def run_match(
    game: str,
    bot_dirs: list[str],
    *,
    sets: int = 3,
    seed: int = 0,
    match_id: str = "local",
    names: list[str] | None = None,
    replay_path: str | None = None,
    summary_path: str | None = None,
    echo_bot_output: bool = False,
) -> str: ...
