"""Configuration from the environment; see `api.env` for which files are read."""

from pydantic_settings import BaseSettings, SettingsConfigDict

from api.env import env_files


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="UCBC_", env_file=env_files(), extra="ignore")

    database_url: str
    blob_url: str
    api_host: str = "127.0.0.1"
    api_port: int = 8000
    forwarded_allow_ips: str = "127.0.0.1"


settings = Settings()
