from pathlib import Path

import click

from ucbc_engine.runner import DEFAULT_MEMORY_BYTES, DEFAULT_STEP_MS


@click.command()
@click.argument("bot_a", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.argument("bot_b", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.option("--game", help="Defaults to the only compiled game.")
@click.option("--sets", default=3, show_default=True)
@click.option("--seed", default=0, show_default=True)
@click.option("--match-id", default="local", show_default=True)
@click.option(
    "--replay", type=click.Path(dir_okay=False, path_type=Path), help="Write the replay here."
)
@click.option(
    "--summary", type=click.Path(dir_okay=False, path_type=Path), help="Write the summary here."
)
@click.option("--show-bot-output", is_flag=True, help="Echo bot output as the match runs.")
@click.option(
    "--step-ms", default=DEFAULT_STEP_MS, show_default=True, help="Time budget per bot step."
)
@click.option(
    "--memory-mb", default=DEFAULT_MEMORY_BYTES >> 20, show_default=True, help="Memory per bot."
)
@click.option(
    "--upload",
    is_flag=True,
    help="Record the match on the API named by UCBC_API_URL/USERNAME/PASSWORD.",
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
    show_bot_output: bool,
    step_ms: int,
    memory_mb: int,
    upload: bool,
) -> None:
    """Play BOT_A against BOT_B. Each is a directory containing main.py."""
    import logging
    import signal

    from ucbc_engine.runner import run_match
    from ucbc_engine.upload import UploadError

    logging.basicConfig(level=logging.INFO if upload else logging.WARNING, format="%(message)s")

    # The match runs in Rust; let Ctrl-C end the process instead of waiting for it.
    signal.signal(signal.SIGINT, signal.SIG_DFL)

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
            step_ms=step_ms,
            memory_bytes=memory_mb * 2**20,
            upload=upload,
        )
    except (RuntimeError, ValueError, UploadError) as e:
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
