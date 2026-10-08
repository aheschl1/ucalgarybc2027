"""The API's settings plus the worker's own, from the same `UCBC_` variables."""

import os
import socket

from pydantic import Field

from api.settings import Settings


class WorkerSettings(Settings):
    worker_slots: int = 1
    worker_poll_s: float = 2.0
    worker_heartbeat_s: float = 5.0
    sets_per_match: int = 3
    # Several heartbeats, so one slow beat does not lose the match.
    worker_lease_s: float = 30.0
    worker_name: str = Field(default_factory=lambda: f"{socket.gethostname()}:{os.getpid()}")


settings = WorkerSettings()
