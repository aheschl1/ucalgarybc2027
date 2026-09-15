"""Configuration from the environment. `ENV=prod` layers `.env.prod` over `.env`; anything
else layers `.env.local` over `.env`. Process variables win over both files."""

import os
from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict

ROOT = Path(__file__).resolve().parents[2]


def _env_files() -> tuple[Path, Path]:
    realm = "prod" if os.environ.get("ENV") == "prod" else "local"
    return ROOT / ".env", ROOT / f".env.{realm}"


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="UCBC_", env_file=_env_files(), extra="ignore")

    database_url: str
    api_host: str = "127.0.0.1"
    api_port: int = 8000
    # Read by `ucbc-api-cli create-admin` only.
    admin_username: str | None = None
    admin_password: str | None = None


settings = Settings()
