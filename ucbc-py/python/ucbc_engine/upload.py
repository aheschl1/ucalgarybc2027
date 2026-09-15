"""Records a match on the platform API as it plays: the match is created before the first
set, each set is posted as it ends, and the result marks the match done."""

import base64
import json
import os
from dataclasses import dataclass
from typing import Any
from urllib import error, request

ENV_VARS = ("UCBC_API_URL", "UCBC_API_USERNAME", "UCBC_API_PASSWORD")


class UploadError(Exception):
    pass


@dataclass(frozen=True)
class Uploader:
    url: str
    username: str
    password: str
    timeout: float = 10.0

    @classmethod
    def from_env(cls) -> "Uploader | None":
        """None unless every variable in ENV_VARS is set."""
        values = [os.environ.get(name) for name in ENV_VARS]
        if not all(values):
            return None
        url, username, password = (v for v in values if v)
        return cls(url, username, password)

    def create_match(
        self, game: str, engine_version: str, names: list[str], config: dict[str, Any]
    ) -> str:
        body = {
            "game": game,
            "engine_version": engine_version,
            "teams": [{"id": i, "name": name} for i, name in enumerate(names)],
            "config": config,
        }
        return str(self._post("/matches", json.dumps(body))["id"])

    def add_set(self, match_id: str, set_json: str) -> None:
        self._post(f"/matches/{match_id}/sets", set_json)

    def complete(self, match_id: str, result: dict[str, Any]) -> None:
        self._post(f"/matches/{match_id}/complete", json.dumps(result))

    def _post(self, path: str, body: str) -> Any:
        auth = base64.b64encode(f"{self.username}:{self.password}".encode()).decode()
        req = request.Request(
            self.url.rstrip("/") + path,
            data=body.encode(),
            method="POST",
            headers={"Content-Type": "application/json", "Authorization": f"Basic {auth}"},
        )
        try:
            with request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read()
        except error.HTTPError as e:
            raise UploadError(f"POST {path}: {e.code} {e.read().decode(errors='replace')}") from e
        except error.URLError as e:
            raise UploadError(f"POST {path}: {e.reason}") from e
        return json.loads(raw) if raw else None
