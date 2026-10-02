import io
import zipfile

from pathlib import Path

import click
import httpx


MAX_ZIP = 1 << 20
MAX_UNPACKED = 8 << 20
MAX_NAME = 64
SERVER = "https://ucbc.andrewheschl.ca/api"

def zip_dir(directory: Path) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for path in sorted(directory.rglob("*")):
            rel = path.relative_to(directory) 
            # Skips any files like __pycache__ or hidden files
            if any(p == "__pycache__" or p.startswith(".") for p in rel.parts):
                continue
            if path.is_file():
                zf.write(path, rel.as_posix())
    return buf.getvalue()

def check_zip(data: bytes) -> None:
    zf = zipfile.ZipFile(io.BytesIO(data))
    if len(data) > MAX_ZIP:
        raise click.ClickException(f"zip file size is larger than {MAX_ZIP >> 20} MiB")    
    if sum(i.file_size for i in zf.infolist()) > MAX_UNPACKED:
       raise click.ClickException(f"zip unpacks to more than {MAX_UNPACKED >> 20} MiB")


@click.command()
@click.argument("bot", type=click.Path(exists=True, file_okay=False, path_type=Path)) # makes sure the input is a directory, not a file
@click.option("--game", required=True, type=click.Choice(["tictactoe", "ucbc2027"])) # works for now, may need to change later (*)
@click.option("--name", help="Defaults to folder name")
@click.option("--test-run", is_flag=True, help="Zip and check without sending.")
def submit(bot: Path, game: str, name: str | None, test_run: bool) -> None:
    """Zip BOT (a directory with main.py at its root) and submit it."""
    if not (bot / "main.py").is_file():
        raise click.ClickException(f"{bot} has no main.py")

    # Zip folder
    zipped = zip_dir(bot)
    if name and len(name) > MAX_NAME:
        raise click.ClickException("name is longer than the max length (64 characters)")    
    check_zip(zipped)

    # If test-run, echo size and file list, then return
    if test_run:
        names = zipfile.ZipFile(io.BytesIO(zipped)).namelist()
        click.echo(f"{len(zipped)} bytes, {len(names)} files")
        for n in names:
            click.echo(f"   {n}")
        return 

    # Upload
    form = {"game": game}
    if name:
        form["name"] = name

    try:
        response = httpx.post(
            f"{SERVER}/submissions",
            files={"file": (f"{bot.name}.zip", zipped, "application/zip")},
            data=form,    
        )
    except httpx.HTTPError as e: 
        raise click.ClickException(f"could not reach {SERVER}: {e}") from e

    if response.status_code != 201:
        raise click.ClickException(response.json().get("detail", response.text))
    body = response.json()
    click.echo(f"Submitted {body['name']} ({body['id']})")
