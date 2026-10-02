"""The lab spawns a few dinos. Each dino wanders until it sees a fossil, then carries
it home."""

import random

from ucbc.games.ucbc2027 import (
    Coord,
    Direction,
    EnvironmentLab,
    ItemViewFossil,
    Ucbc2027Handle,
    UnitViewDino,
    UnitViewLab,
)
from ucbc.handle import ActionError

DINOS = 4
LOOK = 3
"""How far around itself a dino looks for fossils. Every tile looked at is a query,
and queries are what a step's time goes on."""


def step(handle: Ucbc2027Handle) -> None:
    me = handle.me()
    if isinstance(me, UnitViewLab):
        lab(handle, me)
    elif isinstance(me, UnitViewDino):
        dino(handle, me)


def lab(handle: Ucbc2027Handle, me: UnitViewLab) -> None:
    spawned = handle.memory.get("spawned", 0)
    if spawned >= DINOS:
        return
    x0, y0 = me.origin.x, me.origin.y
    for y in range(y0 - 1, y0 + 3):
        for x in range(x0 - 1, x0 + 3):
            try:
                handle.spawn(x, y)
            except ActionError:
                continue
            handle.memory["spawned"] = spawned + 1
            return


def dino(handle: Ucbc2027Handle, me: UnitViewDino) -> None:
    if "home" not in handle.memory:
        handle.memory["home"] = home_tile(handle, me.pos)
        handle.memory["rng"] = random.Random(handle.seed)
    home: Coord = handle.memory["home"]
    if me.held is not None:
        if dist(home, me.pos) <= 1:
            handle.drop(home.x, home.y)
        else:
            go(handle, home)
        return
    fossil = nearest_fossil(handle, me.pos)
    if fossil is None:
        wander(handle)
    elif dist(fossil, me.pos) <= 1:
        handle.grab(fossil.x, fossil.y)
    else:
        go(handle, fossil)


def go(handle: Ucbc2027Handle, target: Coord) -> None:
    try:
        handle.step_toward(target.x, target.y)
    except ActionError:
        wander(handle)  # Blocked: sidestep and try again next turn.


def wander(handle: Ucbc2027Handle) -> None:
    """Keep going one way; pick a new random way when blocked."""
    rng: random.Random = handle.memory["rng"]
    heading = handle.memory.get("heading") or rng.choice(list(Direction))
    try:
        handle.move_dir(heading)
    except ActionError:
        heading = None
    handle.memory["heading"] = heading


def home_tile(handle: Ucbc2027Handle, pos: Coord) -> Coord:
    """A tile of this team's lab next to where the dino was spawned."""
    for y in range(pos.y - 1, pos.y + 2):
        for x in range(pos.x - 1, pos.x + 2):
            if x < 0 or y < 0:
                continue
            env = handle.environment(x, y)
            if isinstance(env, EnvironmentLab) and env.team == handle.team:
                return Coord(x, y)
    raise RuntimeError("spawned away from the lab")


def nearest_fossil(handle: Ucbc2027Handle, pos: Coord) -> Coord | None:
    width, height = handle.width(), handle.height()
    best: Coord | None = None
    for y in range(max(pos.y - LOOK, 0), min(pos.y + LOOK + 1, height)):
        for x in range(max(pos.x - LOOK, 0), min(pos.x + LOOK + 1, width)):
            if isinstance(handle.item(x, y), ItemViewFossil):
                here = Coord(x, y)
                if best is None or dist(here, pos) < dist(best, pos):
                    best = here
    return best


def dist(a: Coord, b: Coord) -> int:
    """Tiles apart, diagonals counting as one."""
    return max(abs(a.x - b.x), abs(a.y - b.y))
