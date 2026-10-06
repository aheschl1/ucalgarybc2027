"""ucbc2027 replays carry each unit's vision

Revision ID: 0013
Revises: 0012
Create Date: 2026-10-06
"""

import gzip
import json
from collections.abc import Sequence
from typing import Any

from sqlalchemy import text

from alembic import op

revision: str = "0013"
down_revision: str | Sequence[str] | None = "0012"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None

# The rules when vision arrived (games/ucbc2027/src/unit.rs), frozen here: a later change
# to them must not rewrite what old replays say.
LAB_VISION = 2
DINO_VISION = 3

KEYS = """
    select s.match_id, s.index from sets s join matches m on m.id = s.match_id
    where m.game = 'ucbc2027'
"""


def vision(unit: dict[str, Any]) -> int:
    if unit["type"] == "lab":
        return LAB_VISION
    level: int = unit["level"]
    return DINO_VISION + (level - 1) // 2


def upgrade() -> None:
    # Every unit in every snapshot gets the vision it had then, so the viewer can rely on
    # the field. Units that already have one are left alone.
    conn = op.get_bind()
    for match_id, index in conn.execute(text(KEYS)).all():
        key = {"m": match_id, "i": index}
        (gz,) = conn.execute(
            text("select replay from sets where match_id = :m and index = :i"), key
        ).one()
        replay = json.loads(gzip.decompress(gz))
        states = [replay["initial_state"], *(tick["state_after"] for tick in replay["ticks"])]
        for state in states:
            for unit in state["units"]:
                unit.setdefault("vision", vision(unit))
        conn.execute(
            text("update sets set replay = :gz where match_id = :m and index = :i"),
            {"gz": gzip.compress(json.dumps(replay).encode(), compresslevel=6), **key},
        )


def downgrade() -> None:
    # The field is only added; a viewer from before it ignores it.
    pass
