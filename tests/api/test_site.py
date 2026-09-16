"""The site serves the API under /api and the built web app at /."""

from pathlib import Path

from fastapi import FastAPI
from httpx import ASGITransport, AsyncClient

from api.api import APP_ROUTES, create_site


def client(site: FastAPI) -> AsyncClient:
    return AsyncClient(transport=ASGITransport(app=site), base_url="http://test")


async def test_site_routes(database_url: str, blob_url: str, tmp_path: Path) -> None:
    (tmp_path / "index.html").write_text("<h1>ucbc</h1>")
    (tmp_path / "assets").mkdir()
    (tmp_path / "assets" / "app.js").write_text("// app")
    site = create_site(database_url, blob_url, static=tmp_path)
    async with site.router.lifespan_context(site), client(site) as c:
        assert (await c.get("/api/health")).json() == {"status": "ok"}
        assert (await c.get("/assets/app.js")).text == "// app"
        r = await c.get("/api/users/me")
        assert r.status_code == 401
        assert "www-authenticate" not in r.headers

        # Every route the app owns is served the shell, so reloading one works.
        for path in APP_ROUTES:
            assert (await c.get(path)).text == "<h1>ucbc</h1>", path

        # Nothing else is: a missing file is a missing file.
        assert (await c.get("/assets/gone.js")).status_code == 404
        assert (await c.get("/nonsense")).status_code == 404


async def test_site_without_a_build(database_url: str, blob_url: str, tmp_path: Path) -> None:
    site = create_site(database_url, blob_url, static=tmp_path / "missing")
    async with site.router.lifespan_context(site), client(site) as c:
        assert (await c.get("/api/health")).status_code == 200
        assert (await c.get("/")).status_code == 404
