"""The lab spawns a few dinos. Each dino wanders until it sees a fossil, then carries
it home. A dino that spots a fossil while its hands are full splits off a level 1
dino to go get it, once per fossil. The split-off dino is born away from the lab and
has to find it by sight before it can deliver.

Every lab and dino runs its own copy of this file, so the module global `bot` holds
that one unit's controller from step to step."""

import random

from ucbc.games.ucbc2027 import (
    Coord,
    Direction,
    EnvironmentLab,
    ItemViewDino,
    ItemViewFossil,
    Ucbc2027Handle,
    UnitViewDino,
    UnitViewLab,
)
from ucbc.handle import ActionError

DINOS = 4
SPLIT_LEVEL = 1  # What the helper that goes for the fossil takes with it.

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
        # A spawned dino is born next to the lab; a split-off one is not, and looks
        # for it from step to step.
        self.home = find_home(handle, me.pos, 1)
        self.rng = random.Random(handle.seed)
        self.heading: Direction | None = None
        self.split_for: set[int] = set()

    def step(self, handle: Ucbc2027Handle) -> None:
        me = handle.me()
        assert isinstance(me, UnitViewDino)  # For the type checker: a dino stays a dino.
        if self.home is None:
            self.home = find_home(handle, me.pos, me.vision)
        if me.held is not None:
            seen = nearest_fossil(handle, me.pos, me.vision)
            if seen is not None and seen[1].id not in self.split_for and self.split_off(handle, me):
                self.split_for.add(seen[1].id)
            if self.home is None:
                self.wander(handle)  # Split off out of sight of the lab: find it.
            elif dist(self.home, me.pos) <= 1:
                handle.drop(self.home.x, self.home.y)
            else:
                self.go(handle, self.home)
            return

        target_level = handle.bones() // 100  # bot heuristic: what level shoudl we aim to be
        # look at every cell around the active bot
        for coord in around(handle, me.pos):
            tileitem = handle.item(coord.x, coord.y)
            # merge based on level heuristic
            if (
                isinstance(tileitem, ItemViewDino)
                and target_level > me.level
                and handle.can_merge(tileitem.id)
            ):
                merged = handle.merge(tileitem.id)
                assert isinstance(merged, UnitViewDino)
                me = merged  # A merge moves this dino onto the other's tile.
                if me.held is not None:
                    return  # And hands over what the other held: deliver it next step.
            # attack weaker dinos
            elif (
                isinstance(tileitem, ItemViewDino)
                and handle.team != tileitem.team
                and me.level > tileitem.level
            ):
                handle.attack(tileitem.id)
                return

        seen = nearest_fossil(handle, me.pos, me.vision)

        if seen is None:
            self.wander(handle)
        elif dist(seen[0], me.pos) <= 1:
            handle.grab(seen[0].x, seen[0].y)
        else:
            self.go(handle, seen[0])

    def split_off(self, handle: Ucbc2027Handle, me: UnitViewDino) -> bool:
        """Split a helper onto the first free tile around, if the rules allow it now.
        The helper is a fresh unit: it runs its own copy of this file from the next
        tick and finds the fossil for itself."""
        for coord in around(handle, me.pos):
            if handle.can_split(coord.x, coord.y, SPLIT_LEVEL):
                handle.split(coord.x, coord.y, SPLIT_LEVEL)
                return True
        return False

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


def find_home(handle: Ucbc2027Handle, pos: Coord, look: int) -> Coord | None:
    """The closest tile of this team's lab within `look` tiles, if any is in sight."""
    width, height = handle.width(), handle.height()
    best: Coord | None = None
    for y in range(max(pos.y - look, 0), min(pos.y + look + 1, height)):
        for x in range(max(pos.x - look, 0), min(pos.x + look + 1, width)):
            env = handle.environment(x, y)
            if isinstance(env, EnvironmentLab) and env.team == handle.team:
                here = Coord(x, y)
                if best is None or dist(here, pos) < dist(best, pos):
                    best = here
    return best


def nearest_fossil(
    handle: Ucbc2027Handle, pos: Coord, look: int
) -> tuple[Coord, ItemViewFossil] | None:
    """The closest fossil within `look` tiles, and where it lies: the dino's vision,
    since a tile any further is refused. Every tile looked at is a query, and queries
    are what a step's time goes on."""
    width, height = handle.width(), handle.height()
    best: tuple[Coord, ItemViewFossil] | None = None
    for y in range(max(pos.y - look, 0), min(pos.y + look + 1, height)):
        for x in range(max(pos.x - look, 0), min(pos.x + look + 1, width)):
            seen = handle.item(x, y)
            if isinstance(seen, ItemViewFossil):
                here = Coord(x, y)
                if best is None or dist(here, pos) < dist(best[0], pos):
                    best = (here, seen)
    return best


def dist(a: Coord, b: Coord) -> int:
    """Tiles apart, diagonals counting as one."""
    return max(abs(a.x - b.x), abs(a.y - b.y))


def around(handle: Ucbc2027Handle, pos: Coord) -> list[Coord]:
    """The 8 tiles around a position, in reading order."""
    results = []
    for y in range(pos.y - 1, pos.y + 2):
        for x in range(pos.x - 1, pos.x + 2):
            if x == pos.x and y == pos.y:
                continue
            if x < 0 or y < 0 or x >= handle.width() or y >= handle.height():
                continue
            results.append(Coord(x, y))
    return results
