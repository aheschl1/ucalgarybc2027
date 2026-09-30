"""Migrations that move existing rows, run against a database of their own."""

import gzip
import json
from collections.abc import Iterator
from uuid import uuid4

import psycopg
import pytest
from psycopg.types.json import Jsonb

from tests.api.conftest import alembic


@pytest.fixture
def fresh_url(database_url: str) -> Iterator[str]:
    """An empty database next to the test one."""
    with psycopg.connect(database_url, autocommit=True) as conn:
        conn.execute("drop database if exists migrations")
        conn.execute("create database migrations")
    yield database_url.rsplit("/", 1)[0] + "/migrations"


def test_teams_backfill(fresh_url: str) -> None:
    alembic(fresh_url, "upgrade", "0006")
    with psycopg.connect(fresh_url, autocommit=True) as conn:
        ids: dict[str, int] = {}
        for name in ["Ann", "Cal", "Bob", "bob", "BOB"]:
            row = conn.execute(
                "insert into users (email, display_name, password_hash) "
                "values (%s, %s, 'x') returning id",
                (f"{name.lower()}{len(ids)}@example.com", name),
            ).fetchone()
            assert row is not None
            ids[name] = row[0]
        # Ann has two submissions, lowercase bob one, the rest none.
        for name in ["Ann", "Ann", "bob"]:
            conn.execute(
                "insert into submissions (id, user_id, name, game, size, sha256) "
                "values (%s, %s, 'bot', 'tictactoe', 1, 'x')",
                (uuid4(), ids[name]),
            )

    alembic(fresh_url, "upgrade", "0007")
    with psycopg.connect(fresh_url) as conn:
        teams = conn.execute(
            "select u.display_name, t.name, t.id from users u join teams t on t.id = u.team_id "
            "order by u.id"
        ).fetchall()
        # Everyone has their own team; the oldest of a clashing name keeps it.
        assert [(d, n) for d, n, _ in teams] == [
            ("Ann", "Ann"),
            ("Cal", "Cal"),
            ("Bob", "Bob"),
            ("bob", "bob 2"),
            ("BOB", "BOB 3"),
        ]
        assert len({id for _, _, id in teams}) == 5
        assert conn.execute("select count(*) from teams").fetchone() == (5,)

        # Every submission is its uploader's team's.
        owners = conn.execute(
            "select u.display_name, s.team_id = u.team_id from submissions s "
            "join users u on u.id = s.user_id order by u.id"
        ).fetchall()
        assert owners == [("Ann", True), ("Ann", True), ("bob", True)]

        nullable = conn.execute(
            "select table_name, is_nullable from information_schema.columns "
            "where column_name = 'team_id' order by table_name"
        ).fetchall()
        assert nullable == [("submissions", "NO"), ("users", "NO")]

    # Every team gets its own join code.
    alembic(fresh_url, "upgrade", "0008")
    with psycopg.connect(fresh_url) as conn:
        codes = conn.execute("select count(distinct join_code) from teams").fetchone()
        assert codes == (5,)

    alembic(fresh_url, "downgrade", "0006")
    with psycopg.connect(fresh_url) as conn:
        assert conn.execute("select to_regclass('teams')").fetchone() == (None,)
        assert conn.execute("select count(*) from submissions").fetchone() == (3,)


def test_submission_bots(fresh_url: str) -> None:
    alembic(fresh_url, "upgrade", "0008")
    a, b = str(uuid4()), str(uuid4())

    def source(bot: str) -> dict[str, str]:
        return {"kind": "submission", "id": bot} if bot in (a, b) else {"kind": "path", "path": bot}

    with psycopg.connect(fresh_url, autocommit=True) as conn:
        ids = {}
        for name, bots in [
            ("paths", ["bots/x", "bots/y"]),
            ("mixed", [a, "bots/y"]),
            ("submissions", [b, a]),
            ("empty", []),
        ]:
            row = conn.execute(
                "insert into matches (origin, game, teams, config, bots, status) "
                "values ('user', 'tictactoe', '[]', '{}', %s, 'done') returning id",
                (Jsonb([source(bot) for bot in bots]),),
            ).fetchone()
            assert row is not None
            ids[name] = row[0]
        conn.execute(
            "insert into sets (match_id, index, first_team, reason, ticks, replay) "
            "values (%s, 0, 0, 'draw', 1, '{}')",
            (ids["paths"],),
        )

    # Matches with a directory go, sets and all; the rest name their submissions in order.
    alembic(fresh_url, "upgrade", "0009")
    with psycopg.connect(fresh_url) as conn:
        rows = conn.execute("select id, bots from matches order by created_at").fetchall()
        assert rows == [(ids["submissions"], [b, a]), (ids["empty"], [])]
        assert conn.execute("select count(*) from sets").fetchone() == (0,)

    alembic(fresh_url, "downgrade", "0008")
    with psycopg.connect(fresh_url) as conn:
        rows = conn.execute("select bots from matches order by created_at").fetchall()
        assert rows == [([source(b), source(a)],), ([],)]


def test_replays_gzipped(fresh_url: str) -> None:
    replay = {"index": 0, "first_team": 0, "initial_state": {"cells": ["empty"] * 9}, "ticks": []}
    alembic(fresh_url, "upgrade", "0009")
    with psycopg.connect(fresh_url, autocommit=True) as conn:
        row = conn.execute(
            "insert into matches (origin, game, teams, config, bots, status) "
            "values ('user', 'tictactoe', '[]', '{}', '[]', 'done') returning id"
        ).fetchone()
        assert row is not None
        conn.execute(
            "insert into sets (match_id, index, first_team, reason, ticks, replay) "
            "values (%s, 0, 0, 'draw', 1, %s)",
            (row[0], Jsonb(replay)),
        )

    # Existing rows are compressed in place and come back as the same JSON.
    alembic(fresh_url, "upgrade", "0010")
    with psycopg.connect(fresh_url) as conn:
        (stored,) = conn.execute("select replay from sets").fetchone() or (None,)
        assert isinstance(stored, bytes)
        assert json.loads(gzip.decompress(stored)) == replay
        (kind,) = conn.execute(
            "select data_type from information_schema.columns "
            "where table_name = 'sets' and column_name = 'replay'"
        ).fetchone() or (None,)
        assert kind == "bytea"

    alembic(fresh_url, "downgrade", "0009")
    with psycopg.connect(fresh_url) as conn:
        assert conn.execute("select replay from sets").fetchone() == (replay,)
