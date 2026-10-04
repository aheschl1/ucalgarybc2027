from datetime import datetime, timedelta
from typing import Any
from uuid import UUID

from psycopg import AsyncConnection
from psycopg.rows import DictRow
from psycopg.types.json import Jsonb

from api.models.matches import (
    MatchConfig,
    MatchOrigin,
    MatchRow,
    SetReplay,
    SetResult,
    TeamInfo,
)

COLUMNS = (
    "id, origin, game, engine_version, teams, config, bots, maps, status, set_wins, winner_team, error, "
    "priority, attempts, max_attempts, claimed_by, claimed_at, heartbeat_at, created_at, "
    "completed_at"
)
QUALIFIED = ", ".join(f"matches.{c}" for c in COLUMNS.split(", "))
SET_RESULT_COLUMNS = "index, first_team, winner_team, reason, detail, ticks"
# A match with a submission the team owns.
OWNED = """
    exists (
        select 1 from jsonb_array_elements_text(matches.bots) b
        join submissions s on s.id = b::uuid
        where s.team_id = %s
    )
"""
# What a team may see on top of that: platform matches. A null team sees everything.
VISIBLE = f"%s::bigint is null or matches.origin = 'platform' or {OWNED}"


class MatchRepo:
    """Matches, which are also the queue. A worker `claim`s a match and holds it through
    `heartbeat` until `complete`, `fail`, or `requeue`. `(id, claimed_at)` is the lease: a
    match whose heartbeat goes stale can be claimed again, and every write by the earlier
    holder then matches no row."""

    def __init__(self, conn: AsyncConnection[DictRow]) -> None:
        self._conn = conn

    async def insert(
        self,
        origin: MatchOrigin,
        game: str,
        teams: list[TeamInfo],
        config: MatchConfig,
        bots: list[UUID],
        maps: list[UUID],
        priority: int,
    ) -> UUID:
        cur = await self._conn.execute(
            "insert into matches (origin, game, teams, config, bots, maps, priority, status) "
            "values (%s, %s, %s, %s, %s, %s, %s, 'queued') returning id",
            (
                origin,
                game,
                Jsonb([t.model_dump() for t in teams]),
                Jsonb(config.model_dump()),
                Jsonb([str(b) for b in bots]),
                Jsonb([str(m) for m in maps]),
                priority,
            ),
        )
        row = await cur.fetchone()
        assert row is not None
        id: UUID = row["id"]
        return id

    async def get(self, match_id: UUID, team_id: int | None = None) -> MatchRow | None:
        """The match, when `team_id` is None or may see it."""
        cur = await self._conn.execute(
            f"select {COLUMNS} from matches where id = %s and ({VISIBLE})",
            (match_id, team_id, team_id),
        )
        row = await cur.fetchone()
        return None if row is None else MatchRow.model_validate(row)

    async def list_recent(
        self,
        team_id: int | None,
        limit: int,
        owned_only: bool = False,
        origin: MatchOrigin | None = None,
        involving: int | None = None,
        active: bool = False,
    ) -> list[MatchRow]:
        """Newest first; what `team_id` may see, or every match when None. `owned_only`
        narrows to matches with one of that team's submissions, `involving` to matches with
        one of that other team's, `origin` to user or platform matches, and `active` to
        matches not yet done."""
        wheres = [f"({VISIBLE})"]
        args: list[Any] = [team_id, team_id]
        if owned_only:
            wheres.append(OWNED)
            args.append(team_id)
        if involving is not None:
            wheres.append(OWNED)
            args.append(involving)
        if origin is not None:
            wheres.append("matches.origin = %s")
            args.append(origin)
        if active:
            wheres.append("matches.status in ('queued', 'running')")
        cur = await self._conn.execute(
            f"select {COLUMNS} from matches where {' and '.join(wheres)} "
            "order by created_at desc limit %s",
            (*args, limit),
        )
        return [MatchRow.model_validate(row) for row in await cur.fetchall()]

    async def claim(self, worker: str, lease: timedelta) -> MatchRow | None:
        """The next queued match, or a running one whose heartbeat is older than `lease`."""
        cur = await self._conn.execute(
            f"""
            with next as (
                select id from matches
                where status = 'queued'
                   or (status = 'running' and heartbeat_at < now() - %s)
                order by priority desc, created_at
                for update skip locked
                limit 1
            )
            update matches set status = 'running', claimed_by = %s, claimed_at = now(),
                heartbeat_at = now(), attempts = attempts + 1
            from next where matches.id = next.id
            returning {QUALIFIED}
            """,
            (lease, worker),
        )
        row = await cur.fetchone()
        return None if row is None else MatchRow.model_validate(row)

    async def heartbeat(self, match_id: UUID, claimed_at: datetime) -> bool:
        return await self._update_held(match_id, claimed_at, "heartbeat_at = now()", ())

    async def requeue(self, match_id: UUID, claimed_at: datetime) -> bool:
        """Back to the queue; the attempt stays counted."""
        return await self._update_held(
            match_id,
            claimed_at,
            "status = 'queued', claimed_by = null, claimed_at = null, heartbeat_at = null",
            (),
        )

    async def fail(self, match_id: UUID, claimed_at: datetime, error: str) -> bool:
        return await self._update_held(
            match_id, claimed_at, "status = 'error', error = %s, completed_at = now()", (error,)
        )

    async def complete(
        self,
        match_id: UUID,
        claimed_at: datetime,
        set_wins: list[int],
        winner_team: int | None,
        engine_version: str,
    ) -> bool:
        return await self._update_held(
            match_id,
            claimed_at,
            "status = 'done', set_wins = %s, winner_team = %s, engine_version = %s, "
            "completed_at = now()",
            (Jsonb(set_wins), winner_team, engine_version),
        )

    async def _update_held(
        self, match_id: UUID, claimed_at: datetime, assignments: str, params: tuple[Any, ...]
    ) -> bool:
        """False when the lease has been taken over."""
        cur = await self._conn.execute(
            f"update matches set {assignments} "
            "where id = %s and claimed_at = %s and status = 'running'",
            (*params, match_id, claimed_at),
        )
        return cur.rowcount == 1

    async def add_set(self, match_id: UUID, replay: SetReplay, replay_key: str) -> None:
        """Stores the set summary and its replay object key."""
        r = replay.result
        await self._conn.execute(
            "insert into sets (match_id, index, first_team, winner_team, reason, detail, ticks, "
            "replay_key) values (%s, %s, %s, %s, %s, %s, %s, %s)",
            (
                match_id,
                r.index,
                r.first_team,
                r.winner_team,
                r.reason,
                r.detail,
                r.ticks,
                replay_key, # store key which is string instead of the gzipped json
            ),
        )

    async def delete_sets(self, match_id: UUID) -> None:
        await self._conn.execute("delete from sets where match_id = %s", (match_id,))

    async def list_set_results(self, match_id: UUID) -> list[SetResult]:
        cur = await self._conn.execute(
            f"select {SET_RESULT_COLUMNS} from sets where match_id = %s order by index",
            (match_id,),
        )
        return [SetResult.model_validate(row) for row in await cur.fetchall()]

    async def get_set_replay(
        self, match_id: UUID, index: int
    ) -> str | None:
        """Returns the replay blob key."""
        cur = await self._conn.execute(
            "select replay_key from sets where match_id = %s and index = %s",
            (match_id, index),
        )
        row = await cur.fetchone()
        if row is None:
            return None
        return row["replay_key"]
