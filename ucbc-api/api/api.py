"""The FastAPI application. `uv run ucbc-api` serves the site: the API under /api and the
web app, when built (`make web`), at /."""

import logging
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path

import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.staticfiles import StaticFiles

from api.blobs import BlobStore
from api.db import create_pool
from api.errors import ApiError
from api.routes import auth, matches, submissions, users
from api.settings import settings

STATIC = Path(__file__).with_name("static")

log = logging.getLogger(__name__)


def create_app(database_url: str | None = None, blob_url: str | None = None) -> FastAPI:
    """The API alone, with its routes at the root."""
    url = database_url or settings.database_url
    blobs = BlobStore(blob_url or settings.blob_url)

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        app.state.pool = create_pool(url)
        await app.state.pool.open()
        app.state.blobs = blobs
        await blobs.ensure_bucket()
        try:
            yield
        finally:
            await app.state.pool.close()

    app = FastAPI(title="UCBC", lifespan=lifespan)
    app.include_router(auth.router)
    app.include_router(users.router)
    app.include_router(matches.router)
    app.include_router(submissions.router)
    app.add_exception_handler(ApiError, api_error)

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ok"}

    return app


def create_site(
    database_url: str | None = None, blob_url: str | None = None, static: Path = STATIC
) -> FastAPI:
    """The API mounted at /api and the web app served at /."""
    api = create_app(database_url, blob_url)

    @asynccontextmanager
    async def lifespan(site: FastAPI) -> AsyncIterator[None]:
        # A mounted app's lifespan does not run on its own.
        async with api.router.lifespan_context(api):
            yield

    site = FastAPI(lifespan=lifespan, openapi_url=None)
    site.mount("/api", api)
    if static.is_dir():
        site.mount("/", StaticFiles(directory=static, html=True))
    else:
        log.warning("no web app at %s; serving the API only", static)
    return site


async def api_error(_request: Request, exc: Exception) -> JSONResponse:
    assert isinstance(exc, ApiError)
    return JSONResponse({"detail": str(exc)}, status_code=exc.status_code)


def main() -> None:
    uvicorn.run(create_site(), host=settings.api_host, port=settings.api_port)
