"""The FastAPI application. `uv run ucbc-api` serves it with uvicorn."""

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from api.db import create_pool
from api.errors import ApiError
from api.routes import matches, users
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
    app.include_router(matches.router)
    app.add_exception_handler(ApiError, api_error)

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ok"}

    return app


async def api_error(_request: Request, exc: Exception) -> JSONResponse:
    assert isinstance(exc, ApiError)
    return JSONResponse({"detail": str(exc)}, status_code=exc.status_code)


def main() -> None:
    uvicorn.run(create_app(), host=settings.api_host, port=settings.api_port)
