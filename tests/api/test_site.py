"""The site serves the API under /api and the built web app at /."""

from pathlib import Path

from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient

from api.api import create_site


def client(site: FastAPI) -> AsyncClient:
    return AsyncClient(transport=ASGITransport(app=site), base_url="http://test")


async def test_site_routes(database_url: str, blob_url: str, tmp_path: Path) -> None:
    (tmp_path / "index.html").write_text("<h1>ucbc</h1>")
    site = create_site(database_url, blob_url, static=tmp_path)
    async with site.router.lifespan_context(site), client(site) as c:
        assert (await c.get("/api/health")).json() == {"status": "ok"}
        assert (await c.get("/")).text == "<h1>ucbc</h1>"
        r = await c.get("/api/users/me")
        assert r.status_code == 401
        assert "www-authenticate" not in r.headers


async def test_site_without_a_build(database_url: str, blob_url: str, tmp_path: Path) -> None:
    site = create_site(database_url, blob_url, static=tmp_path / "missing")
    async with site.router.lifespan_context(site), client(site) as c:
        assert (await c.get("/api/health")).status_code == 200
        assert (await c.get("/")).status_code == 404
