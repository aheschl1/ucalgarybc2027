"""`ENV=prod` layers `.env.prod` over `.env`; anything else layers `.env.local` over `.env`.
Process variables win over both files."""

import os
from pathlib import Path
import dotenv

ROOT = Path(__file__).resolve().parents[2]


def env_files() -> tuple[Path, Path]:
    realm = "prod" if os.environ.get("ENV") == "prod" else "local"
    return ROOT / ".env", ROOT / f".env.{realm}"

def load_env() -> None:
    """Load the environment variables from the files, if they exist."""
    for path in env_files():
        if path.exists():
            dotenv.load_dotenv(path, override=True)