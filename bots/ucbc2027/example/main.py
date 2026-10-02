"""The lab spawns a few dinos. Each dino wanders until it sees a fossil, then carries
it home.

Every lab and dino runs its own copy of this file, so the module global `bot` holds
that one unit's controller from step to step."""

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


bot: "Lab | Dino | None" = None


def step(handle: Ucbc2027Handle) -> None:
    global bot
    if bot is None:
        me = handle.me()
        bot = Lab(me) if isinstance(me, UnitViewLab) else Dino(handle, me)
    bot.step(handle)


class Lab:
    def __init__(self, me: UnitViewLab) -> None:
        self.origin = me.origin
        self.spawned = 0

    def step(self, handle: Ucbc2027Handle) -> None:
        if self.spawned >= DINOS:
            return
        for y in range(self.origin.y - 1, self.origin.y + 3):
            for x in range(self.origin.x - 1, self.origin.x + 3):
                try:
                    handle.spawn(x, y)
                except ActionError:
                    continue
                self.spawned += 1
                return


class Dino:
    def __init__(self, handle: Ucbc2027Handle, me: UnitViewDino) -> None:
        self.home = home_tile(handle, me.pos)
        self.rng = random.Random(handle.seed)
        self.heading: Direction | None = None

    def step(self, handle: Ucbc2027Handle) -> None:
        me = handle.me()
        assert isinstance(me, UnitViewDino)  # For the type checker: a dino stays a dino.
        if me.held is not None:
            if dist(self.home, me.pos) <= 1:
                handle.drop(self.home.x, self.home.y)
            else:
                self.go(handle, self.home)
            return
        fossil = nearest_fossil(handle, me.pos)
        if fossil is None:
            self.wander(handle)
        elif dist(fossil, me.pos) <= 1:
            handle.grab(fossil.x, fossil.y)
        else:
            self.go(handle, fossil)

    def go(self, handle: Ucbc2027Handle, target: Coord) -> None:
        try:
            handle.step_toward(target.x, target.y)
        except ActionError:
            self.wander(handle)  # Blocked: sidestep and try again next turn.

    def wander(self, handle: Ucbc2027Handle) -> None:
        """Keep going one way; pick a new random way when blocked."""
        heading = self.heading or self.rng.choice(list(Direction))
        try:
            handle.move_dir(heading)
            self.heading = heading
        except ActionError:
            self.heading = None


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
