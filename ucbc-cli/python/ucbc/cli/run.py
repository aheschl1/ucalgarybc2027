from pathlib import Path
from typing import Any

import click

from ucbc.runner import DEFAULT_MEMORY_BYTES, DEFAULT_STEP_MS


@click.command()
@click.argument("bot_a", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.argument("bot_b", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.option("--game", help="Defaults to the only compiled game.")
@click.option("--sets", default=1, show_default=True)
@click.option("--seed", default=0, show_default=True)
@click.option("--match-id", default="local", show_default=True)
@click.option(
    "--replay",
    type=click.Path(dir_okay=False, path_type=Path),
    help="Write the replay here; .gz or .json",
)
@click.option(
    "--summary", type=click.Path(dir_okay=False, path_type=Path), help="Write the summary here."
)
@click.option(
    "--map",
    "map_files",
    multiple=True,
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    help="Play on this map file instead of the game's standard one. Once for every set, "
    "or once per set, in order.",
)
@click.option(
    "--game-config",
    "game_config_json",
    metavar="JSON",
    help="A JSON object for the game, or @FILE to read one. --map sets its maps key.",
)
@click.option(
    "--max-ticks",
    type=click.IntRange(min=1),
    help="End each set after this many ticks; the game decides the result. "
    "Defaults to the engine's limit.",
)
@click.option("--view", is_flag=True, help="Open the replay in the viewer after the match.")
@click.option("--show-bot-output", is_flag=True, help="Echo bot output as the match runs.")
@click.option("--no-verbose", is_flag=True, help="Hide tick progress and time estimates.")
@click.option(
    "--step-ms",
    default=DEFAULT_STEP_MS,
    show_default=True,
    help="Budget per bot step, in ms of bot time: instructions, not wall clock.",
)
@click.option(
    "--memory-mb", default=DEFAULT_MEMORY_BYTES >> 20, show_default=True, help="Memory per bot."
)
def run(
    bot_a: Path,
    bot_b: Path,
    game: str | None,
    sets: int,
    seed: int,
    match_id: str,
    replay: Path | None,
    summary: Path | None,
    map_files: tuple[Path, ...],
    game_config_json: str | None,
    max_ticks: int | None,
    view: bool,
    show_bot_output: bool,
    no_verbose: bool,
    step_ms: int,
    memory_mb: int,
) -> None:
    """Play BOT_A against BOT_B. Each is a directory containing main.py."""
    import base64
    import json
    import signal
    import tempfile

    from ucbc import viewer
    from ucbc.runner import run_match

    # The match runs in Rust; let Ctrl-C end the process instead of waiting for it.
    signal.signal(signal.SIGINT, signal.SIG_DFL)

    if view:
        viewer.check_built()
    if view and replay is None:
        # Left behind: the process ends on Ctrl-C while serving it.
        replay = Path(tempfile.mkdtemp(prefix="ucbc-")) / "replay.json.gz"

    if map_files and len(map_files) not in (1, sets):
        raise click.UsageError(
            f"--map was given {len(map_files)} times; give it once, or once per set ({sets})"
        )

    game_config: dict[str, Any] | None = None
    if game_config_json is not None:
        if game_config_json.startswith("@"):
            try:
                game_config_json = Path(game_config_json[1:]).read_text()
            except OSError as e:
                raise click.BadParameter(str(e), param_hint="--game-config") from e
        try:
            game_config = json.loads(game_config_json)
        except json.JSONDecodeError as e:
            raise click.BadParameter(f"not JSON: {e}", param_hint="--game-config") from e
        if not isinstance(game_config, dict):
            raise click.BadParameter("must be a JSON object", param_hint="--game-config")
    if map_files:
        if game_config is not None and "maps" in game_config:
            raise click.UsageError("--map and --game-config both set maps; give one")
        game_config = game_config or {}
        game_config["maps"] = [base64.b64encode(m.read_bytes()).decode() for m in map_files]

    names = [bot_a.name, bot_b.name]
    try:
        result = run_match(
            bot_a,
            bot_b,
            game=game,
            sets=sets,
            seed=seed,
            match_id=match_id,
            names=names,
            replay_path=replay,
            summary_path=summary,
            echo_bot_output=show_bot_output,
            verbose=not no_verbose,
            step_ms=step_ms,
            memory_bytes=memory_mb * 2**20,
            max_ticks=max_ticks,
            game_config=game_config,
        )
    except (RuntimeError, ValueError) as e:
        raise click.ClickException(str(e)) from e

    for s in result["sets"]:
        winner = (
            "draw"
            if s["winner_team"] is None
            else f"{names[s['winner_team']]} wins by {s['reason']}"
        )
        detail = f" ({s['detail']})" if s.get("detail") else ""
        click.echo(f"Set {s['index'] + 1}: {winner} after {s['ticks']} ticks{detail}")
    score = ", ".join(f"{n} {w}" for n, w in zip(names, result["set_wins"]))
    if result["winner_team"] is None:
        click.echo(f"Result: tie ({score})")
    else:
        click.echo(f"Result: {names[result['winner_team']]} wins ({score})")
    if replay:
        click.echo(f"Replay: {replay}")
    if summary:
        click.echo(f"Summary: {summary}")
    if view and replay:
        viewer.open_viewer(replay)
