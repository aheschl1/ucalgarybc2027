"""`ucbc-worker`: plays queued matches until told to stop."""

import asyncio
import logging
import signal
from datetime import timedelta

import click
from psycopg import AsyncConnection
from psycopg.rows import DictRow
from psycopg_pool import AsyncConnectionPool

from api.blobs import BlobStore
from api.db import DBConnection, create_pool
from api.services import matches
from api.services.matches import LostLease
from worker.match import MatchFailed, StartFailed, play
from worker.settings import settings

log = logging.getLogger("worker")


class Worker:
    def __init__(
        self,
        pool: AsyncConnectionPool[AsyncConnection[DictRow]],
        blobs: BlobStore,
        *,
        name: str,
        slots: int,
        poll_s: float,
        heartbeat_s: float,
        lease: timedelta,
    ) -> None:
        self.pool = pool
        self.blobs = blobs
        self.name = name
        self.slots = slots
        self.poll_s = poll_s
        self.heartbeat_s = heartbeat_s
        self.lease = lease

    async def serve(self, stop: asyncio.Event) -> None:
        """Plays `slots` matches at a time until `stop` is set; running matches finish."""
        await asyncio.gather(*(self._slot(stop) for _ in range(self.slots)))

    async def _slot(self, stop: asyncio.Event) -> None:
        while not stop.is_set():
            try:
                async with self.pool.connection() as conn:
                    await conn.set_autocommit(True)
                    played = await self.run_once(DBConnection(conn))
            except Exception:
                log.exception("slot failed")
                played = False
            if not played:
                await _wait(stop, self.poll_s)

    async def run_once(self, db: DBConnection) -> bool:
        """Plays the next queued match; False when the queue is empty."""
        match = await matches.claim_match(db, self.name, self.lease)
        if match is None:
            return False
        log.info("match %s: attempt %d", match.id, match.attempts)
        try:
            try:
                replay = await play(
                    match, self.blobs, lambda: matches.heartbeat(db, match), self.heartbeat_s
                )
            except MatchFailed as e:
                log.warning("match %s: %s", match.id, e)
                await matches.fail_match(db, match, str(e))
            except StartFailed as e:
                log.error("match %s: could not start the engine: %s", match.id, e)
                await matches.requeue_match(db, match)
            else:
                await matches.finish_match(db, match, replay, self.blobs)
                log.info("match %s: done", match.id)
        except LostLease as e:
            log.warning("%s", e)
        return True


async def _wait(stop: asyncio.Event, seconds: float) -> None:
    try:
        await asyncio.wait_for(stop.wait(), seconds)
    except TimeoutError:
        pass


async def _serve() -> None:
    pool = create_pool(settings.database_url, size=settings.worker_slots)
    await pool.open()
    stop = asyncio.Event()
    loop = asyncio.get_running_loop()

    def request_stop() -> None:
        log.info("stopping after the running matches")
        stop.set()
        # A second signal ends the process at once.
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.remove_signal_handler(sig)

    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, request_stop)
    worker = Worker(
        pool,
        BlobStore(settings.blob_url),
        name=settings.worker_name,
        slots=settings.worker_slots,
        poll_s=settings.worker_poll_s,
        heartbeat_s=settings.worker_heartbeat_s,
        lease=timedelta(seconds=settings.worker_lease_s),
    )
    log.info("%s: %d slots", worker.name, worker.slots)
    try:
        await worker.serve(stop)
    finally:
        await pool.close()


@click.command()
def main() -> None:
    """Play queued matches until SIGTERM; running matches finish first."""
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    asyncio.run(_serve())
