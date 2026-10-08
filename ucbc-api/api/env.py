"""`ENV=prod` layers `.env.prod` over `.env`; anything else layers `.env.local` over `.env`.
Process variables win over both files."""

import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def env_files() -> tuple[Path, Path]:
    realm = "prod" if os.environ.get("ENV") == "prod" else "local"
    return ROOT / ".env", ROOT / f".env.{realm}"
