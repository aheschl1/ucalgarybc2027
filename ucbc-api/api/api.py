"""The FastAPI application. `uv run ucbc-api` serves it with uvicorn."""

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI

from api.db import create_pool
from api.routes import users
from api.settings import settings


def create_app(database_url: str | None = None) -> FastAPI:
    url = database_url or settings.database_url

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        app.state.pool = create_pool(url)
        await app.state.pool.open()
        try:
            yield
        finally:
            await app.state.pool.close()

    app = FastAPI(title="UCBC", lifespan=lifespan)
    app.include_router(users.router)

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ok"}

    return app


def main() -> None:
    uvicorn.run(create_app(), host=settings.api_host, port=settings.api_port)
