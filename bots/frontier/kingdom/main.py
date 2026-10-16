"""Kingdom: a walled town that farms, mines, smelts, and raises the Monument.

The commander never leaves its shelter beside the base: it spawns workers, gives each a
role, and relays the base's stock and the veins the team has found. The base sits in a
ring of walls and gates with towers inside; hidden spikes cover the approaches. Workers
farm wheat and apples around the pond and wells, fill granaries, cut wood and stone for
the base, build the town from a fixed plan, study and craft at the desk, workbench, and
smelters, mine and dig out veins, man the towers, raise the Monument, and once the base is
stocked, hoard what they gather in chests and walls in the wild, which counts for points.
Each unit runs its own copy of this file: module globals belong to one unit."""

import base64
import random
from collections.abc import Callable, Iterator
from contextlib import suppress

from ucbc.games.frontier import (
    STEPS,
    WALKABLE,
    Building,
    Crop,
    Dir,
    FrontierHandle,
    Item,
    Kind,
    Observation,
    Recipe,
    Tiles,
    beside,
    towards,
)
from ucbc.handle import ActionError

Pos = tuple[int, int]
Costs = dict[Item, int]

CARRY = 20
GRASS, FOREST, ROCK, WATER, DIRT, COAL, COPPER, IRON, FARMLAND = range(9)
FREE = bytes(t in WALKABLE for t in range(256))
DIGGABLE = bytes(t in WALKABLE or t == ROCK for t in range(256))
DIGITS = b"01" + b"1" * 254
MASK = [bytes(tile) + b"\x01" + bytes(255 - tile) for tile in range(9)]
WET = b"0" * WATER + b"1" + b"0" * (255 - WATER)
BUILDINGS = list(Building)
ITEMS = list(Item)
BASE = BUILDINGS.index(Building.BASE)
GATE = BUILDINGS.index(Building.GATE)
TRAP = BUILDINGS.index(Building.SPIKE_TRAP)
CHEST = BUILDINGS.index(Building.CHEST)
TOWER = BUILDINGS.index(Building.TOWER)
WELL = BUILDINGS.index(Building.WELL)
MONUMENT = BUILDINGS.index(Building.MONUMENT)
DRILL = BUILDINGS.index(Building.MECH_DRILL)
DESK = BUILDINGS.index(Building.RESEARCH_DESK)
SHOPS = (Building.SMELTER, Building.WORKBENCH)
APPLE_TREE = list(Crop).index(Crop.APPLE_TREE)
DIRS = list(STEPS.items())
COMPASS = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]
FOOD = (Item.WHEAT, Item.APPLE)

WOODMAN, MASON, FARMER, BUILDER, SMITH, GUARD, COALER, COPPERER, IRONER, HOARDER = range(1, 11)
MINES = {COALER: COAL, COPPERER: COPPER, IRONER: IRON}
TOOLS = {COPPERER: Item.STONE_PICK, IRONER: Item.COPPER_PICK}
ORE = {COAL: Item.COAL, COPPER: Item.COPPER_ORE, IRON: Item.IRON_ORE}
EARLY = [WOODMAN, BUILDER, WOODMAN, BUILDER, MASON, WOODMAN, FARMER, GUARD, FARMER, SMITH, GUARD]
STUDY = [
    Item.STONE,
    Item.WOOD,
    Item.COAL,
    Item.COPPER_ORE,
    Item.CHARCOAL,
    Item.COPPER_INGOT,
    Item.IRON_ORE,
    Item.IRON_INGOT,
    Item.GEAR,
    Item.APPLE,
]
GOODS = {
    Item.COPPER_INGOT,
    Item.IRON_INGOT,
    Item.GEAR,
    Item.STONE_PICK,
    Item.COPPER_PICK,
    Item.COPPER_SWORD,
    Item.COPPER_ARMOUR,
}
BROADCAST = [Item.WHEAT, Item.APPLE, Item.WOOD, Item.STONE]

WOOD_STOCK, STONE_STOCK = 140, 90
ORE_STOCK = {COAL: 80, COPPER: 60, IRON: 60}
CAPS = {Item.WOOD: 180, Item.STONE: 120, Item.COAL: 120, Item.COPPER_ORE: 80, Item.IRON_ORE: 80}
HOARD_FROM = 18
POP_CAP = 80
CHEST_ROOM = 40
RESERVE = 15
APPLES = 31
BASE_FULL = 920

# The town, in the base's frame: x is mirrored for the right-hand team, so +x faces the
# other team, and the pond lies north. Inside the wall the buildings stand on even rows and
# columns, so the odd ones are streets; the commander's shelter is the one exception: walls
# beside its post and a tower behind it, which it shoots from. Guards wait beside the base
# until there are towers to man.
POST = (0, 1)
SHELTER = [(-1, 1), (1, 1)]
TOWERS = [((0, 2), (1, 2)), ((2, -2), (3, -2))]
IDLE = [(-3, -3), (3, -3)]
WATCH = [(2, 0), (0, -2), (-2, 0)]
GRANARIES = [(-2, -5), (2, -5), (-4, -12), (4, -12), (-5, -8), (5, -8)]
WELLS = [(9, -9), (-9, -9), (0, -16)]
RADIO, MONUMENT_AT, SMELTER2 = (-8, 3), (-8, 0), (-8, -3)


def ring() -> list[Pos]:
    """The wall around the town, east (toward the other team) first: all gates, which
    cost a little more than walls, but let the team through wherever the land outside
    allows."""
    east = [(4, y) for y in range(-4, 5)]
    west = [(-4, y) for y in range(-4, 5)]
    return east + [(x, y) for y in (-4, 4) for x in range(-3, 4)] + west


def trap_spots() -> list[Pos]:
    """Hidden spikes in the two rings just outside the wall, east first; the farm and the
    pond to the north are left clear."""
    spots = []
    for d in (5, 6):
        spots += [(d, dy) for dy in range(-d, d + 1)]
        spots += [(dx, d) for dx in range(-d, d)]
        spots += [(dx, -d) for dx in range(-d, d + 1) if abs(dx) > 5]
    for d in (5, 6):
        spots += [(-d, dy) for dy in range(-d, d + 1)]
    return list(dict.fromkeys(spots))


TRAPS = trap_spots()


def town() -> list[tuple[Building, int, int]]:
    """The plan, in the order it's built."""
    B = Building
    parts: list[tuple[Building, list[Pos]]] = [
        (B.WOOD_WALL, SHELTER),
        (B.RESEARCH_DESK, [(-2, 2)]),
        (B.WORKBENCH, [(-2, -2)]),
        (B.GATE, ring()),
        (B.TOWER, [tower for tower, _ in TOWERS]),
        (B.SMELTER, [(2, 2)]),
        (B.CHEST, GRANARIES),
        (B.SMELTER, [SMELTER2]),
        (B.RADIO_TOWER, [RADIO]),
        (B.WELL, WELLS),
        (B.SPIKE_TRAP, TRAPS),
        (B.MONUMENT, [MONUMENT_AT]),
    ]
    return [(kind, x, y) for kind, spots in parts for x, y in spots]


class Mind:
    def __init__(self) -> None:
        self.role = 0
        self.born = -1
        self.id = -1
        self.base: Pos = (0, 0)
        self.m = 1
        self.size = 0
        self.rng = random.Random()
        self.plan: list[tuple[Building, Pos]] = []
        self.granaries: list[Pos] = []
        self.ring: list[Pos] = []
        # Walking.
        self.path: list[Pos] = []
        self.path_key: object = None
        self.stuck = 0
        self.fails: dict[object, int] = {}
        self.progress: tuple[object, int, int] = (None, 0, 0)
        """The far walk under way, the closest it has come, and when."""
        self.bad: set[Pos] = set()
        # What the team knows.
        self.costs: dict[Building, Costs] = {}
        self.hp: dict[Building, int] = {}
        self.tower = 0
        """The range of a tower."""
        self.recipes: dict[Recipe, tuple[Building, Costs]] = {}
        self.made_at: dict[Building, set[Item]] = {}
        self.limits: dict[Building, Costs] = {}
        """At each shop, the most of an item worth keeping there: six runs' worth."""
        self.known_at = -(10**9)
        self.studied: set[Item] = set()
        # The base and stores.
        self.stock: Costs = {}
        self.stock_at = -(10**6)
        self.heard_at = -(10**6)
        self.base_total = 0
        self.base_full_until = 0
        self.need: Costs = {}
        """What the commander still lacks to rebuild the base."""
        self.need_at = -(10**6)
        self.empty: dict[Pos, int] = {}
        self.full_until: dict[Pos, int] = {}
        self.full: set[Pos] = set()
        # Roles.
        self.unload = ""
        self.carrying: Costs = {}
        self.target: tuple[Building, Pos] | None = None
        self.veins: dict[int, Pos] = {}
        self.explore: Pos | None = None
        self.explore_until = 0
        self.explored = 0
        self.drills = 0
        self.tries = 0
        self.field: list[Pos] = []
        self.field_set: set[Pos] = set()
        self.field_at = -(10**9)
        self.shop: dict[Pos, tuple[Costs, int]] = {}
        self.made: dict[Recipe, int] = {}
        self.skip: dict[Pos, int] = {}
        self.owed: dict[Pos, Costs] = {}
        self.post: Pos | None = None
        # The commander.
        self.alive: dict[int, tuple[int, int]] = {}
        self.assigned: list[tuple[int, int, int]] = []
        self.spawned = 0


mind = Mind()


def at(dx: int, dy: int) -> Pos:
    return (mind.base[0] + mind.m * dx, mind.base[1] + dy)


def dist(a: Pos, b: Pos) -> int:
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]))


def dir_to(me: Pos, p: Pos) -> Dir | None:
    for d, (dx, dy) in DIRS:
        if (me[0] + dx, me[1] + dy) == p:
            return d
    return None


def plus(*parts: Costs) -> Costs:
    out: Costs = {}
    for part in parts:
        for i, n in part.items():
            out[i] = out.get(i, 0) + n
    return out


class View:
    """One step's observation, decoded as far as the step needs."""

    def __init__(self, seen: Observation) -> None:
        self.seen = seen
        self.tick = seen.tick
        self.tiles = Tiles(seen)
        self.data = self.tiles.data
        self.ox, self.oy = seen.origin.x, seen.origin.y
        self.w, self.h = seen.width, seen.height
        self.me: Pos = (seen.me.pos.x, seen.me.pos.y)
        self.team = seen.me.team
        self.carried = {s.item: s.count for s in seen.me.carrying}
        self.load = sum(self.carried.values())
        self.enemies: list[tuple[Pos, int]] = []
        self.mates: list[Pos] = []
        r = seen.units
        for i in range(0, len(r), 6):
            p = (r[i + 3], r[i + 4])
            if r[i + 1] == self.team:
                self.mates.append(p)
            else:
                self.enemies.append((p, r[i + 5]))
        self.occupied = set(self.mates) | {p for p, _ in self.enemies}
        self._blds: dict[Pos, tuple[int, int, int, int]] | None = None
        self._piles: dict[Pos, Costs] | None = None

    def inside(self, p: Pos) -> bool:
        return 0 <= p[0] - self.ox < self.w and 0 <= p[1] - self.oy < self.h

    def buildings(self) -> dict[Pos, tuple[int, int, int, int]]:
        """Position: (team, building index, hp, work left)."""
        if self._blds is None:
            b = self.seen.buildings
            self._blds = {
                (b[i + 3], b[i + 4]): (b[i + 1], b[i + 2], b[i + 5], b[i + 6])
                for i in range(0, len(b), 7)
            }
        return self._blds

    def piles(self) -> dict[Pos, Costs]:
        if self._piles is None:
            r = self.seen.piles
            self._piles = {}
            for i in range(0, len(r), 4):
                self._piles.setdefault((r[i], r[i + 1]), {})[ITEMS[r[i + 2]]] = r[i + 3]
        return self._piles

    def own(self, kind: int, finished: bool = True) -> list[Pos]:
        return [
            p
            for p, (team, k, _, left) in self.buildings().items()
            if team == self.team and k == kind and (left == 0) == finished
        ]

    def food(self) -> int:
        return self.carried.get(Item.WHEAT, 0) + self.carried.get(Item.APPLE, 0)


def stores_beside(v: View) -> Iterator[tuple[Pos, Dir, Costs, Costs, int]]:
    """The team's storage and sites next to this unit: where, which way, what it holds,
    what it's owed, and when its job ends (-1 for none)."""
    for s in v.seen.stores:
        p = (s.pos.x, s.pos.y)
        d = dir_to(v.me, p)
        if d is not None:
            stored = {st.item: st.count for st in s.stored}
            owed = {st.item: st.count for st in s.owed}
            yield p, d, stored, owed, s.job.done_at if s.job else -1


def known(handle: FrontierHandle, v: View, every: int) -> None:
    """Refreshes what the team knows: building costs and recipes."""
    if v.tick - mind.known_at < every:
        return
    k = handle.known()
    mind.costs = {b.building: {s.item: s.count for s in b.cost} for b in k.buildings}
    mind.hp = {b.building: b.hp for b in k.buildings}
    mind.tower = max((b.tower or 0) for b in k.buildings)
    mind.recipes = {r.recipe: (r.at, {s.item: s.count for s in r.inputs}) for r in k.recipes}
    mind.made_at, mind.limits = {}, {}
    for r in k.recipes:
        mind.made_at.setdefault(r.at, set()).update(s.item for s in r.outputs)
        limit = mind.limits.setdefault(r.at, {})
        for s in r.inputs:
            limit[s.item] = max(limit.get(s.item, 0), 6 * s.count)
    mind.known_at = v.tick


# Walking


def grid(v: View, table: bytes, mates: bool) -> bytearray:
    """Walkable tiles in view, around buildings and units (only the other team's unless
    `mates`); the team's own gates and traps are walkable."""
    free = bytearray(v.data.translate(table))
    w, ox, oy = v.w, v.ox, v.oy
    for x, y in v.occupied if mates else (p for p, _ in v.enemies):
        free[(y - oy) * w + x - ox] = 0
    b = v.seen.buildings
    for i in range(0, len(b), 7):
        if not (b[i + 1] == v.team and b[i + 6] == 0 and b[i + 2] in (GATE, TRAP)):
            free[(b[i + 4] - oy) * w + b[i + 3] - ox] = 0
    return free


def bits(cells: bytes | bytearray, w: int, h: int) -> int:
    """A 0/1 byte per tile as one integer, a bit per tile, rows `w + 1` bits apart; the
    spare bit ending each row keeps shifts from wrapping."""
    text = cells.translate(DIGITS)
    rows = b"0".join(text[y * w : (y + 1) * w] for y in range(h)) + b"0"
    return int(rows[::-1], 2)


def bfs(
    v: View,
    goal: bytes | bytearray,
    near: bool,
    reach: int = 0,
    toward: Pos | None = None,
    table: bytes = FREE,
    mates: bool = True,
) -> list[Pos] | None:
    """A shortest walk to a tile in `goal`, or beside one when `near`, within `reach` of
    here if given; failing that, to the reachable tile nearest `toward` if given. The next
    step is last; [] if already there, None if there's no walk. The search spreads a whole
    ring of tiles at a time as bit masks."""
    w, h, row = v.w, v.h, v.w + 1
    mx, my = v.me[0] - v.ox, v.me[1] - v.oy
    left = bits(grid(v, table, mates), w, h)
    if reach:
        x0, x1 = max(mx - reach, 0), min(mx + reach + 1, w)
        span = ((1 << (x1 - x0)) - 1) << x0
        box = 0
        for y in range(max(my - reach, 0), min(my + reach + 1, h)):
            box |= span << (y * row)
        left &= box
    target = bits(goal, w, h)
    if near:
        target = target << 1 | target >> 1 | target << row | target >> row
    start = 1 << (my * row + mx)
    if target & start:
        return []
    left &= ~start
    rings = [start]
    ring = start
    hit = 0
    while ring and not hit:
        ring = (ring << 1 | ring >> 1 | ring << row | ring >> row) & left
        left ^= ring
        rings.append(ring)
        hit = ring & target
    if not hit:
        if toward is None:
            return None
        reached = [i for i, c in enumerate(bin(sum(rings))[:1:-1]) if c == "1"]
        tx, ty = toward[0] - v.ox, toward[1] - v.oy
        c = min(reached, key=lambda i: max(abs(i % row - tx), abs(i // row - ty)))
        if c == my * row + mx:
            return None
        k = next(k for k, r in enumerate(rings) if r >> c & 1)
        del rings[k + 1 :]
    else:
        c = (hit & -hit).bit_length() - 1
    path = []
    for prev in reversed(rings[:-1]):
        path.append((c % row + v.ox, c // row + v.oy))
        c = next(nb for nb in (c - 1, c + 1, c - row, c + row) if nb >= 0 and prev >> nb & 1)
    return path


def step_path(handle: FrontierHandle, v: View) -> bool:
    """The next step of the walk; waits a tick or two for a unit in the way."""
    if not mind.path:
        return False
    nx = mind.path[-1]
    d = dir_to(v.me, nx)
    if d is not None:
        try:
            handle.move(d)
            mind.path.pop()
            mind.stuck = 0
            return True
        except ActionError:
            pass
    mind.stuck += 1
    if d is not None and nx in v.occupied and mind.stuck < 3:
        return True
    mind.path = []
    return False


def go(
    handle: FrontierHandle,
    v: View,
    key: object,
    goal: Callable[[], bytes | bytearray],
    near: bool = False,
    reach: int = 0,
    toward: Pos | None = None,
) -> bool:
    """Follows the walk for `key`, or plans one; True if it moved or is already there. A
    walk blocked by teammates is planned through them, to wait for them to move; a walk
    that can't be planned isn't tried again for a few ticks."""
    if mind.path_key == key and step_path(handle, v):
        return True
    if mind.stuck > 3:
        mind.stuck = 0
        return wiggle(handle)
    if mind.fails.get(key, 0) > v.tick:
        return False
    mind.path_key = key
    g = goal()
    path = None
    if any(g) or toward is not None:
        path = bfs(v, g, near, reach, toward)
        if path is None:
            path = bfs(v, g, near, 0, toward, mates=False)
    if path is None:
        mind.path = []
        mind.fails[key] = v.tick + 3
        if len(mind.fails) > 64:
            mind.fails = {k: t for k, t in mind.fails.items() if t > v.tick}
        return False
    mind.path = path
    return not path or step_path(handle, v)


def wiggle(handle: FrontierHandle) -> bool:
    dirs = [d for d, _ in DIRS]
    mind.rng.shuffle(dirs)
    for d in dirs:
        with suppress(ActionError):
            handle.move(d)
            return True
    return False


def marks(v: View, targets: list[Pos]) -> bytearray:
    goal = bytearray(v.w * v.h)
    for x, y in targets:
        if v.inside((x, y)):
            goal[(y - v.oy) * v.w + x - v.ox] = 1
    return goal


def go_beside(handle: FrontierHandle, v: View, targets: list[Pos], key: object) -> bool:
    if not targets:
        return False
    near = min(dist(t, v.me) for t in targets)
    return go(handle, v, key, lambda: beside(v.tiles, targets), False, near + 5 if near < 10 else 0)


def go_to(handle: FrontierHandle, v: View, target: Pos, key: object, radius: int = 0) -> bool:
    """Toward a tile anywhere: through the view to it, to the view's edge on its side, or
    as close as the view allows. False, marking the tile bad, after a minute without
    getting closer."""
    tx = min(max(target[0], v.ox + 1), v.ox + v.w - 2)
    ty = min(max(target[1], v.oy + 1), v.oy + v.h - 2)
    if (tx, ty) != target:
        radius = max(radius, 3)

    def goal() -> bytearray:
        g = bytearray(v.w * v.h)
        for y in range(max(ty - radius - v.oy, 0), min(ty + radius - v.oy + 1, v.h)):
            for x in range(max(tx - radius - v.ox, 0), min(tx + radius - v.ox + 1, v.w)):
                g[y * v.w + x] = 1
        return g

    k, d = (key, target), dist(v.me, target)
    if mind.progress[0] != k or d < mind.progress[1]:
        mind.progress = (k, d, v.tick)
    if v.tick - mind.progress[2] > 60:
        mind.bad.add(target)
        mind.progress = (None, 0, 0)
        wiggle(handle)
        return False
    if not go(handle, v, k, goal, False, 0, target):
        towards(handle, v.me, target)
    return True


def adjacent(v: View, goal: bytes | bytearray) -> Dir | None:
    """The way to a neighbouring tile marked in `goal`."""
    for d, (dx, dy) in DIRS:
        x, y = v.me[0] + dx - v.ox, v.me[1] + dy - v.oy
        if 0 <= x < v.w and 0 <= y < v.h and goal[y * v.w + x]:
            return d
    return None


# Messages: a tag, then records. The commander says b"C" and R (a unit's id in 3 bytes and
# its role), S (BROADCAST and the base's total, 2 bytes each), B (wood and stone still
# needed to rebuild the base) and V records; a worker says b"H", its role, and V records
# (a vein's tile, then x and y in 2 bytes each).

RECORD = {b"R": 5, b"S": 3 + 2 * len(BROADCAST), b"B": 3, b"V": 6}


def encode(data: bytes) -> str:
    return base64.b64encode(data).decode()


def vein_records() -> list[bytes]:
    return [
        b"V" + bytes([tile]) + x.to_bytes(2, "little") + y.to_bytes(2, "little")
        for tile, (x, y) in mind.veins.items()
    ]


def learn_vein(tile: int, p: Pos) -> None:
    if p in mind.bad:
        return
    old = mind.veins.get(tile)
    if old is None or dist(p, mind.base) < dist(old, mind.base):
        mind.veins[tile] = p


def read(data: bytes, i: int, v: View) -> None:
    def num(j: int) -> int:
        return int.from_bytes(data[j : j + 2], "little")

    while i + RECORD.get(data[i : i + 1], len(data)) <= len(data):
        tag = data[i : i + 1]
        if tag == b"R" and int.from_bytes(data[i + 1 : i + 4], "little") == mind.id:
            if mind.role != data[i + 4]:
                mind.path = []
            mind.role = data[i + 4]
        elif tag == b"S":
            for k, item in enumerate(BROADCAST):
                mind.stock[item] = num(i + 1 + 2 * k)
            mind.base_total = num(i + 1 + 2 * len(BROADCAST))
            mind.heard_at = v.tick
        elif tag == b"B":
            mind.need = {Item.WOOD: data[i + 1], Item.STONE: data[i + 2]}
            mind.need_at = v.tick
        elif tag == b"V":
            learn_vein(data[i + 1], (num(i + 2), num(i + 4)))
        i += RECORD[tag]


def hear(v: View, commander: bool) -> None:
    for msg in v.seen.heard:
        data = base64.b64decode(msg.data)
        if commander and data[:1] == b"H" and len(data) >= 2:
            mind.alive[msg.sender] = (data[1], v.tick)
            read(data, 2, v)
        elif not commander and data[:1] == b"C":
            read(data, 1, v)


def scan_veins(v: View) -> None:
    """Remembers a vein tile of each kind in view that a unit can stand beside."""
    w, data = v.w, v.data
    for tile in (COAL, COPPER, IRON):
        i = data.find(tile)
        for _ in range(60):
            if i < 0:
                break
            x = i % w
            sides = (i - w, i + w, i - 1 if x else -1, i + 1 if x + 1 < w else -1)
            if any(0 <= j < len(data) and FREE[data[j]] for j in sides):
                learn_vein(tile, (x + v.ox, i // w + v.oy))
                break
            i = data.find(tile, i + 1)
    i = data.find(FOREST)
    for _ in range(60):
        if i < 0:
            break
        p = (i % w + v.ox, i // w + v.oy)
        if dist(p, mind.base) > HOARD_FROM + 2:
            learn_vein(FOREST, p)
            break
        i = data.find(FOREST, i + 1)


# The base


def base_store(v: View) -> tuple[Dir, Costs] | None:
    """The way to the finished base, if it's beside, and what it holds."""
    b = v.buildings().get(mind.base)
    if b is None or b[1] != BASE or b[3] > 0:
        return None
    for p, d, stored, _, _ in stores_beside(v):
        if p == mind.base:
            return d, stored
    return None


def visit_base(handle: FrontierHandle, v: View, keep: Costs, want: Costs) -> bool:
    """Beside the base, unloads all but `keep` and takes up to `want`; elsewhere heads
    there. False once there's nothing left to do there."""
    if v.seen.base is None:
        return clear_base(handle, v)
    here = base_store(v)
    if here is None:
        return errand(v, keep, want) and go_home(handle, v)
    d, stored = here
    mind.stock, mind.stock_at = stored, v.tick
    for item, n in v.carried.items():
        if n <= keep.get(item, 0) or capped(v, item):
            continue
        extra = n - keep.get(item, 0)
        try:
            handle.put(d, item, extra)
            return True
        except ActionError:
            mind.base_full_until = v.tick + 150
    room = CARRY - v.load
    for item, n in want.items():
        short = min(n - v.carried.get(item, 0), room)
        if item == Item.WHEAT:
            if mind.role != FARMER:
                short = min(short, stored.get(Item.WHEAT, 0) + stored.get(Item.APPLE, 0) - RESERVE)
            if not stored.get(Item.WHEAT):
                item = Item.APPLE
                spare = stored.get(Item.APPLE, 0) - (APPLES if v.seen.me.hungry_at > v.tick else 0)
                short = min(short, spare)
        if short > 0 and stored.get(item):
            with suppress(ActionError):
                handle.take(d, item, short)
                return True
    return False


def capped(v: View, item: Item) -> bool:
    return mind.stock.get(item, 0) >= CAPS.get(item, 1000) or mind.base_full_until > v.tick


def errand(v: View, keep: Costs, want: Costs) -> bool:
    """Whether the base, as last seen, has room for what this unit would leave there or
    holds what it wants."""
    if any(n > keep.get(i, 0) and not capped(v, i) for i, n in v.carried.items()):
        return True
    food = mind.stock.get(Item.WHEAT, 0) + mind.stock.get(Item.APPLE, 0)
    spare = food > (0 if mind.role == FARMER else RESERVE)
    return any(
        spare if i == Item.WHEAT else bool(mind.stock.get(i)) or mind.stock_at < 0
        for i, n in want.items()
        if n > v.carried.get(i, 0)
    )


def go_home(handle: FrontierHandle, v: View) -> bool:
    if dist(v.me, mind.base) > 14:
        return go_to(handle, v, mind.base, "far-home", 2)

    def sides() -> bytearray:
        goal = beside(v.tiles, [mind.base])
        post = at(*POST)
        if v.inside(post):
            goal[(post[1] - v.oy) * v.w + post[0] - v.ox] = 0
        return goal

    if go(handle, v, "home", sides):
        return True
    if dist(v.me, mind.base) <= 3:
        return wiggle(handle)
    towards(handle, v.me, mind.base)
    return True


def clear_base(handle: FrontierHandle, v: View) -> bool:
    """With the base destroyed, leaves on its tile just what the commander still needs to
    rebuild it, and carries off the rest."""
    d = dir_to(v.me, mind.base)
    if d is None:
        return go_home(handle, v)
    pile = v.piles().get(mind.base, {})
    need = mind.need if v.tick - mind.need_at < 50 else mind.costs.get(Building.BASE, {})
    extra = {i: pile.get(i, 0) - need.get(i, 0) for i in set(pile) | set(need)}
    with suppress(ActionError):
        for item, n in extra.items():
            if n < 0 and v.carried.get(item):
                handle.put(d, item, min(-n, v.carried[item]))
                return True
        for item, n in extra.items():
            if n > 0 and v.load < CARRY:
                handle.take(d, item, min(n, CARRY - v.load))
                return True
            if n > 0:
                junk = max(v.carried, key=lambda i: (i not in FOOD, v.carried[i]))
                return drop(handle, v, junk, v.carried[junk], tuple(WALKABLE))
    return False


def idle(handle: FrontierHandle, v: View) -> None:
    """Out of the way, inside the wall."""
    spot = at(*IDLE[mind.id % len(IDLE)])
    if v.me != spot:
        go(handle, v, ("idle", spot), lambda: marks(v, [spot]))


def eat(handle: FrontierHandle, v: View) -> bool:
    """Food from a granary on the way home with some in it, or else from the base; False
    when no food is known anywhere."""
    want = food_want(mind.role)
    for p, d, stored, owed, _ in stores_beside(v):
        if p in mind.granaries and not owed:
            meal = Item.WHEAT if stored.get(Item.WHEAT) else Item.APPLE
            if not stored.get(meal):
                mind.empty[p] = v.tick + 150
            elif v.food() < want:
                with suppress(ActionError):
                    handle.take(d, meal, want - v.food())
                    return True
    chests = v.own(CHEST)
    granaries = mind.granaries
    built = [g for g in granaries if g in chests] if v.inside(granaries[0]) else granaries
    home = dist(v.me, mind.base)
    open_ = [g for g in built if mind.empty.get(g, 0) < v.tick and dist(g, v.me) < home]
    if home > 4 and open_:
        g = min(open_, key=lambda g: dist(g, v.me))
        if go_beside(handle, v, [g], ("granary", g)):
            return True
        mind.empty[g] = v.tick + 30
    base_food = mind.stock.get(Item.WHEAT, 0) + mind.stock.get(Item.APPLE, 0)
    fresh = v.tick - max(mind.heard_at, mind.stock_at) < 100
    if fresh and base_food <= (0 if mind.role == FARMER else RESERVE):
        return False
    return visit_base(handle, v, keeps(v), {Item.WHEAT: want})


def drop(
    handle: FrontierHandle, v: View, item: Item, n: int, ground: tuple[int, ...] = (FARMLAND, DIRT)
) -> bool:
    """Leaves items on the ground beside, by default on farmland or bare dirt, where they
    are in nobody's way."""
    blds = v.buildings()
    for d, (dx, dy) in DIRS:
        q = (v.me[0] + dx, v.me[1] + dy)
        free = q not in v.occupied and q not in blds and q != mind.base
        if free and v.tiles[q] in ground:
            with suppress(ActionError):
                handle.put(d, item, n)
                return True
    return False


def fill_granary(handle: FrontierHandle, v: View, items: Costs) -> bool:
    for p, d, stored, owed, _ in stores_beside(v):
        if p in mind.granaries and not owed:
            held = sum(stored.values())
            if held < CHEST_ROOM:
                for item, n in items.items():
                    if n > 0 and (item in FOOD or held < CHEST_ROOM - 10):
                        with suppress(ActionError):
                            handle.put(d, item, n)
                            mind.empty.pop(p, None)
                            return True
            mind.full_until[p] = v.tick + 100
    chests = v.own(CHEST)
    room = [g for g in mind.granaries if mind.full_until.get(g, 0) < v.tick and g in chests]
    if room:
        g = min(room, key=lambda g: dist(g, v.me))
        if go_beside(handle, v, [g], ("granary", g)):
            return True
        mind.full_until[g] = v.tick + 30
    return False


def needs_food(v: View) -> bool:
    left = v.seen.me.hungry_at - v.tick
    left += 100 * v.carried.get(Item.WHEAT, 0) + 150 * v.carried.get(Item.APPLE, 0)
    return left < 2 * dist(v.me, mind.base) + 80


def food_want(role: int) -> int:
    if role == HOARDER:
        return 9
    return 3 if role in (FARMER, BUILDER, SMITH, GUARD) else 6


def keeps(v: View) -> Costs:
    keep = {Item.WHEAT: food_want(mind.role), Item.APPLE: food_want(mind.role)}
    tool = TOOLS.get(mind.role)
    if tool:
        keep[tool] = 1
    for weapon in (Item.COPPER_SWORD, Item.COPPER_ARMOUR, Item.IRON_SWORD, Item.IRON_ARMOUR):
        keep[weapon] = 1
    return keep


def resupply(
    handle: FrontierHandle, v: View, extra: Costs | None = None, food: bool = True
) -> bool:
    keep = keeps(v)
    fed = v.seen.me.hungry_at - v.tick + 100 * v.food()
    want = {Item.WHEAT: food_want(mind.role) if fed < 300 else 0}
    if not food:
        keep[Item.WHEAT] = keep[Item.APPLE] = want[Item.WHEAT] = 0
    if extra:
        keep.update(extra)
        want.update(extra)
    return visit_base(handle, v, keep, want)


# Fighting


def attack_adjacent(handle: FrontierHandle, v: View) -> bool:
    for (x, y), _ in sorted(v.enemies, key=lambda e: e[1]):
        if abs(x - v.me[0]) + abs(y - v.me[1]) == 1:
            with suppress(ActionError):
                handle.attack(x, y)
                return True
    return False


def fall_back(handle: FrontierHandle, v: View) -> bool:
    """Back inside the wall while the enemies close by outnumber the team's units there."""
    if mind.role == GUARD or dist(v.me, mind.base) <= 3:
        return False
    foes = sum(1 for p, _ in v.enemies if dist(p, v.me) <= 4)
    return foes > sum(1 for p in v.mates if dist(p, v.me) <= 4) and go_home(handle, v)


def shoot(handle: FrontierHandle, v: View) -> bool:
    """From beside a tower, hits the weakest enemy in its range."""
    for t in v.own(TOWER):
        if abs(t[0] - v.me[0]) + abs(t[1] - v.me[1]) == 1:
            for (x, y), _ in sorted(v.enemies, key=lambda e: e[1]):
                if dist((x, y), t) <= mind.tower:
                    with suppress(ActionError):
                        handle.attack(x, y)
                        return True
    return False


def guard(handle: FrontierHandle, v: View) -> None:
    """Mans a tower inside the wall, or with no tower free, waits beside the base; the
    guards meet raiders that get inside together."""
    for weapon in (Item.COPPER_SWORD, Item.COPPER_ARMOUR):
        fetch = mind.stock.get(weapon) and not v.carried.get(weapon)
        if fetch and dist(v.me, mind.base) < 8 and resupply(handle, v, {weapon: 1}):
            return
    if shoot(handle, v):
        return
    towers = v.own(TOWER)
    posts = [at(*stand) for tower, stand in TOWERS if at(*tower) in towers]
    if mind.post not in posts or (mind.post in v.occupied and mind.post != v.me):
        free = [p for p in posts if p not in v.occupied or p == v.me]
        mind.post = free[mind.id % len(free)] if free else None
    inside = [p for p, _ in v.enemies if dist(p, mind.base) <= 3]
    if inside:
        target = min(inside, key=lambda p: dist(p, v.me))
        go_beside(handle, v, [target], ("chase", target))
        return
    spot = mind.post or at(*WATCH[mind.id % len(WATCH)])
    if v.me != spot:
        if not go(handle, v, ("post", spot), lambda: marks(v, [spot])):
            help_monument(handle, v)
        return
    if v.tick - mind.stock_at > 400:
        resupply(handle, v)


# Entry


def step(handle: FrontierHandle) -> None:
    seen = handle.observe()
    v = View(seen)
    if mind.born < 0:
        mind.born = v.tick
        mind.id = seen.me.id
        mind.size = seen.size
        base = seen.base or seen.me.pos
        mind.base = (base.x, base.y)
        mind.m = 1 if base.x < seen.size // 2 else -1
        mind.rng = random.Random(seen.me.id)
        mind.plan = [(k, at(dx, dy)) for k, dx, dy in town()]
        mind.granaries = [at(*g) for g in GRANARIES]
        mind.ring = [at(*p) for p in ring()]
        known(handle, v, 0)
    if seen.me.kind == Kind.COMMANDER:
        commander(handle, v)
        return
    hear(v, False)
    if mind.role == 0:
        if v.tick - mind.born < 8:
            return
        mind.role = WOODMAN
    if (v.tick + mind.id) % 20 == 0:
        with suppress(ActionError):
            handle.say(encode(b"H" + bytes([mind.role]) + b"".join(vein_records())))
    if (v.tick + mind.id) % 8 == 0:
        scan_veins(v)
    if attack_adjacent(handle, v) or fall_back(handle, v):
        return
    hauling = mind.role == BUILDER and v.load - v.food() >= 15
    if needs_food(v) and not (hauling and v.seen.me.hungry_at - v.tick > 20):
        load = v.load - v.food()
        far = dist(v.me, mind.base) > HOARD_FROM
        spare = v.seen.me.hungry_at - v.tick + 100 * v.food() > 1.5 * dist(v.me, mind.base) + 60
        if mind.role == HOARDER and load >= 10 and far and spare and stash(handle, v):
            return
        if eat(handle, v):
            return
        if not v.food() and mind.role != FARMER:
            farmer(handle, v)
            return
    food = mind.stock.get(Item.WHEAT, 0) + mind.stock.get(Item.APPLE, 0)
    famine = v.tick > 400 and food < 15 and v.tick - mind.heard_at < 100
    if famine and mind.role in (HOARDER, MASON, COALER, COPPERER, IRONER):
        farmer(handle, v)
    else:
        ROLES.get(mind.role, woodman)(handle, v)


# Commander


def commander(handle: FrontierHandle, v: View) -> None:
    hear(v, True)
    if v.tick % 8 == 0:
        scan_veins(v)
    acted = attack_adjacent(handle, v)
    post = at(*POST)
    here = base_store(v)
    there = acted or v.me == post or go(handle, v, "post", lambda: marks(v, [post]))
    if not there and here is None:
        go_beside(handle, v, [mind.base], "base")
    if not acted and here is None:
        rebuild(handle, v)
    elif not acted and here is not None:
        d, stored = here
        mind.stock = stored
        food = v.food()
        if food < 5:
            item = Item.WHEAT if stored.get(Item.WHEAT) else Item.APPLE
            with suppress(ActionError):
                handle.take(d, item, 5 - food)
                acted = True
        if not acted and not shoot(handle, v):
            spawn(handle, v, stored)
    near = (FARMER, BUILDER, SMITH, GUARD)
    mind.alive = {
        k: a for k, a in mind.alive.items() if v.tick - a[1] < (300 if a[0] in near else 1500)
    }
    msg = b"C"
    for uid, role, t in mind.assigned:
        if v.tick - t < 6:
            msg += b"R" + uid.to_bytes(3, "little") + bytes([role])
    msg += b"S" + b"".join(
        min(mind.stock.get(i, 0), 65535).to_bytes(2, "little") for i in BROADCAST
    )
    msg += min(sum(mind.stock.values()), 65535).to_bytes(2, "little")
    if v.seen.base is None:
        cost = mind.costs.get(Building.BASE, {})
        need = (max(0, cost.get(i, 0) - v.carried.get(i, 0)) for i in (Item.WOOD, Item.STONE))
        msg += b"B" + bytes(need)
    for record in vein_records():
        if len(msg) + len(record) <= 64:
            msg += record
    with suppress(ActionError):
        handle.say(encode(msg))


def rebuild(handle: FrontierHandle, v: View) -> None:
    """Rebuilds a destroyed base: unloads all but its cost onto its tile, takes the cost
    from what the base spilled there, and once the workers have cleared the rest, builds."""
    d = dir_to(v.me, mind.base)
    b = v.buildings().get(mind.base)
    if d is None or (b is not None and b[0] != v.team):
        return
    cost = mind.costs.get(Building.BASE, {})
    pile = v.piles().get(mind.base, {})
    with suppress(ActionError):
        if b is not None or (not pile and all(v.carried.get(i, 0) >= n for i, n in cost.items())):
            handle.build(d, Building.BASE)
            return
        for item, n in v.carried.items():
            if n > cost.get(item, 0):
                handle.put(d, item, n - cost.get(item, 0))
                return
        for item, n in cost.items():
            short = n - v.carried.get(item, 0)
            if short > 0 and pile.get(item):
                handle.take(d, item, min(short, pile[item]))
                return


def spawn(handle: FrontierHandle, v: View, stored: Costs) -> None:
    """A worker while the farms can feed one more and the base can spare the wood; once
    the early workers are out, the wall gets its wood first."""
    pop = len(mind.alive)
    wood = stored.get(Item.WOOD, 0)
    food = stored.get(Item.WHEAT, 0) + stored.get(Item.APPLE, 0)
    blds = v.buildings()
    walled = not any(
        v.tiles[p] in (GRASS, DIRT, FOREST) and blds.get(p, (-1, 0, 0, 1))[::3] != (v.team, 0)
        for p in mind.ring
    )
    reserve = 0 if mind.spawned < len(EARLY) else 25 if walled else 60
    farmers = sum(1 for role, _ in mind.alive.values() if role == FARMER)
    need_farmer = mind.spawned >= len(EARLY) and farmers < 2 and food >= 4 and wood >= 10
    if not need_farmer and (wood < 10 + reserve or food < 10 + pop // 3):
        return
    farmland = len(v.seen.soil) // 3
    c = v.seen.crops
    trees = sum(1 for i in range(2, len(c), 4) if c[i] == APPLE_TREE)
    fed = 8 + farmland * 2 // 5 + 3 * trees
    gap = 2 if mind.spawned < len(EARLY) else 12
    if mind.assigned and v.tick - mind.assigned[-1][2] < gap:
        return
    if pop >= min(POP_CAP, fed) and not need_farmer:
        return
    role = FARMER if need_farmer else pick_role(pop)
    try:
        uid = handle.spawn()
    except ActionError:
        return
    mind.spawned += 1
    mind.alive[uid] = (role, v.tick)
    mind.assigned = [a for a in mind.assigned if v.tick - a[2] < 6] + [(uid, role, v.tick)]


def pick_role(pop: int) -> int:
    if mind.spawned < len(EARLY):
        return EARLY[mind.spawned]
    counts: dict[int, int] = {}
    for role, _ in mind.alive.values():
        counts[role] = counts.get(role, 0) + 1
    mining = 0 if pop < 12 else 1 if pop < 25 else 2
    target = {
        FARMER: max(2, pop // 4 + 1),
        BUILDER: 2 if pop < 20 else 3,
        WOODMAN: 3 if pop < 20 else 2 + pop // 8,
        MASON: 2 if pop < 30 else 3,
        SMITH: 1,
        GUARD: 1 if pop < 12 else 3 if pop < 25 else 5,
        COALER: mining + (pop >= 25),
        COPPERER: mining,
        IRONER: mining,
    }
    short = max(target, key=lambda r: target[r] - counts.get(r, 0))
    return short if target[short] > counts.get(short, 0) else HOARDER


# Gathering and hoarding


def resource_goal(v: View, tile: int, away: int) -> bytearray:
    """The view's tiles of `tile`, but none within `away` of the base."""
    goal = bytearray(v.data.translate(MASK[tile]))
    for p in v.buildings():
        goal[(p[1] - v.oy) * v.w + p[0] - v.ox] = 0
    if away:
        bx, by = mind.base[0] - v.ox, mind.base[1] - v.oy
        x0, x1 = max(bx - away, 0), min(bx + away + 1, v.w)
        for y in range(max(by - away, 0), min(by + away + 1, v.h)):
            if x0 < x1:
                goal[y * v.w + x0 : y * v.w + x1] = bytes(x1 - x0)
    return goal


def harvest(handle: FrontierHandle, v: View, key: object, goal: bytearray) -> bool:
    """Gathers from a tile marked in `goal` beside, or walks to one."""
    d = adjacent(v, goal)
    if d is not None:
        try:
            handle.gather(d)
        except ActionError:
            return False
        mind.path = []
        return True
    return go(handle, v, key, lambda: goal, True)


def gather(handle: FrontierHandle, v: View, tile: int, item: Item, hoard: bool) -> None:
    room = CARRY - v.load
    if not v.carried.get(item):
        mind.unload = ""
    if room <= 0 or (room < 2 and v.carried.get(item, 0) > 0) or mind.unload:
        if not mind.unload or base_full(v):
            mind.unload = "stash" if hoard or base_full(v) else "base"
        if mind.unload != "stash" and resupply(handle, v):
            return
        mind.unload = "stash"
        if stash(handle, v):
            return
        mind.unload = ""
        if room <= 0:
            if not resupply(handle, v) and not drop(handle, v, item, v.carried.get(item, 0)):
                wiggle(handle)
            return
    away = HOARD_FROM if hoard else 0
    if harvest(handle, v, ("res", tile, away), resource_goal(v, tile, away)):
        return
    known = mind.veins.get(tile)
    if known is not None and tile == FOREST:
        if dist(v.me, known) > 3 and go_to(handle, v, known, "forest", 2):
            return
        mind.bad.add(known)
        mind.veins.pop(tile, None)
    explore(handle, v)


def explore(handle: FrontierHandle, v: View) -> None:
    """Out in widening rings around the base, one compass point after another."""
    stuck = mind.explore in mind.bad
    if mind.explore is None or v.tick > mind.explore_until or dist(v.me, mind.explore) < 3 or stuck:
        k = mind.explored
        mind.explored += 1
        rings = max(1, min(192, mind.size // 3) // 32)
        r = 32 * (1 + k // 8 % rings)
        dx, dy = COMPASS[(k + mind.id) % 8]
        x, y = mind.base[0] + dx * r, mind.base[1] + dy * r
        mid = mind.size // 2
        x = min(x, mid - 12) if mind.m == 1 else max(x, mid + 12)
        mind.explore = (min(max(x, 1), mind.size - 2), min(max(y, 1), mind.size - 2))
        mind.explore_until = v.tick + 2 * r + 50
    go_to(handle, v, mind.explore, "explore", 2)


def base_full(v: View) -> bool:
    heard = v.tick - mind.heard_at < 300 and mind.base_total >= BASE_FULL
    return heard or mind.base_full_until > v.tick


def away() -> Pos:
    """A spot well clear of the base on this unit's side of it."""
    dx, dy = COMPASS[mind.id % 8]
    x = min(max(mind.base[0] + dx * (HOARD_FROM + 8), 1), mind.size - 2)
    return (x, min(max(mind.base[1] + dy * (HOARD_FROM + 8), 1), mind.size - 2))


def stash(handle: FrontierHandle, v: View) -> bool:
    """Into a chest nearby with room, a chest site, or a new chest or stone wall on the
    lattice where nothing is in anyone's way. False when there is too little to stash."""
    blds = v.buildings()
    for p, d, stored, owed, _ in stores_beside(v):
        b = blds.get(p)
        if b is None or b[1] != CHEST or p in mind.granaries:
            continue
        if owed:
            for item, n in owed.items():
                if v.carried.get(item):
                    with suppress(ActionError):
                        handle.put(d, item, n)
                        return True
            continue
        if sum(stored.values()) >= CHEST_ROOM:
            mind.full.add(p)
            continue
        for item, n in v.carried.items():
            if item not in FOOD and n > 0:
                with suppress(ActionError):
                    handle.put(d, item, n)
                    return True
    sites = v.own(CHEST, finished=False)
    walls = v.own(BUILDINGS.index(Building.STONE_WALL), finished=False)
    for p in sites + walls:
        side = dir_to(v.me, p)
        if side is not None:
            with suppress(ActionError):
                handle.build(side, Building.CHEST if p in sites else Building.STONE_WALL)
                return True
    goods = sum(n for item, n in v.carried.items() if item not in FOOD)
    chests = [p for p in v.own(CHEST) if p not in mind.full]
    if goods and (sites or chests):
        near = min(sites + chests, key=lambda p: dist(p, v.me))
        if dist(near, v.me) <= 8 and go_beside(handle, v, [near], ("chest", near)):
            return True
    wood, stone = v.carried.get(Item.WOOD, 0), v.carried.get(Item.STONE, 0)
    chest = mind.costs.get(Building.CHEST, {}).get(Item.WOOD, 10)
    wall = mind.costs.get(Building.STONE_WALL, {}).get(Item.STONE, 6)
    if wood < chest and stone < wall:
        return False
    if dist(v.me, mind.base) <= HOARD_FROM:
        return go_to(handle, v, away(), "away", 3)
    kind = Building.CHEST if wood >= chest else Building.STONE_WALL
    piles = v.piles()
    for d, (dx, dy) in DIRS:
        p = (v.me[0] + dx, v.me[1] + dy)
        if lattice(v, p) and p not in v.occupied and p not in blds and p not in piles:
            with suppress(ActionError):
                handle.build(d, kind)
                return True
    spots = [
        (x, y)
        for y in range(v.me[1] - 7, v.me[1] + 8)
        for x in range(v.me[0] - 7, v.me[0] + 8)
        if lattice(v, (x, y)) and (x, y) not in blds and (x, y) not in piles
    ]
    return go_beside(handle, v, spots, "chest-spot") or wiggle(handle)


def lattice(v: View, p: Pos) -> bool:
    """Chests and walls in the wild go on every fifth tile, never two side by side or
    corner to corner, so they never wall anyone in."""
    return (p[0] + 2 * p[1]) % 5 == 0 and v.tiles[p] in (GRASS, DIRT)


def farm_forest(v: View) -> bytearray:
    """Forest where the farm should spread: north of the wall, around the pond."""
    goal = resource_goal(v, FOREST, 0)
    bx, by = mind.base[0] - v.ox, mind.base[1] - v.oy
    x0, x1 = max(bx - 8, 0), min(bx + 9, v.w)
    y0, y1 = max(by - 14, 0), min(by - 4, v.h)
    out = bytearray(len(goal))
    for y in range(y0, y1):
        if x0 < x1:
            out[y * v.w + x0 : y * v.w + x1] = goal[y * v.w + x0 : y * v.w + x1]
    return out


def hoarder(handle: FrontierHandle, v: View) -> None:
    """Fills chests in the wild, unless the base runs short of wood; where no forest is
    known it quarries stone into walls instead."""
    short = mind.stock.get(Item.WOOD, 0) < 100 and v.tick - mind.heard_at < 1500
    stone = v.carried.get(Item.STONE, 0)
    forest = FOREST in mind.veins or FOREST in v.data
    if stone or (not forest and ROCK in v.data and dist(v.me, mind.base) > HOARD_FROM):
        gather(handle, v, ROCK, Item.STONE, True)
        return
    gather(handle, v, FOREST, Item.WOOD, not (short and dist(v.me, mind.base) <= 40))


def woodman(handle: FrontierHandle, v: View) -> None:
    hoard = mind.stock_at >= 0 and mind.stock.get(Item.WOOD, 0) >= WOOD_STOCK
    hoard |= mind.base_total >= 800 and v.tick - mind.heard_at < 300
    if v.load and not hoard and v.carried.get(Item.WOOD, 0) == 0 and CARRY - v.load < 3:
        resupply(handle, v)
        return
    if not hoard and CARRY - v.load > 1 and harvest(handle, v, "farm-forest", farm_forest(v)):
        return
    gather(handle, v, FOREST, Item.WOOD, hoard)


def mason(handle: FrontierHandle, v: View) -> None:
    crowded = mind.base_total >= 800 and v.tick - mind.heard_at < 300
    if crowded or (mind.stock_at >= 0 and mind.stock.get(Item.STONE, 0) >= STONE_STOCK):
        gather(handle, v, ROCK, Item.STONE, True)
        return
    if v.carried.get(Item.WOOD, 0) and CARRY - v.load < 2:
        resupply(handle, v)
        return
    gather(handle, v, ROCK, Item.STONE, False)


# Farming


def compute_field(v: View) -> None:
    """Farmland-to-be: watered grass, dirt, or farmland near the base, outside the wall."""
    w, h = v.w, v.h
    water = bytearray(v.data.translate(WET))
    for p in v.own(WELL):
        water[(p[1] - v.oy) * w + p[0] - v.ox] = ord("1")
    rows = []
    for y in range(h):
        m = int(water[y * w : (y + 1) * w][::-1], 2)
        rows.append(m | m << 1 | m << 2 | m << 3 | m << 4 | m >> 1 | m >> 2 | m >> 3 | m >> 4)
    wet = []
    for y in range(h):
        m = 0
        for r in rows[max(y - 4, 0) : y + 5]:
            m |= r
        wet.append(m)
    tilled = (GRASS, DIRT, FARMLAND, FOREST, ROCK)
    skip = set(TRAPS) | set(GRANARIES) | set(WELLS) | {RADIO, MONUMENT_AT, SMELTER2}
    field = []
    for dy in range(-16, 14):
        for dx in range(-14, 15):
            if max(abs(dx), abs(dy)) < 5 or (dx, dy) in skip:
                continue
            p = at(dx, dy)
            x, y = p[0] - v.ox, p[1] - v.oy
            if 0 <= x < w and 0 <= y < h and wet[y] >> x & 1 and v.data[y * w + x] in tilled:
                field.append(p)
    mind.field, mind.field_set = field, set(field)
    mind.field_at = v.tick


def farmer(handle: FrontierHandle, v: View) -> None:
    if (not mind.field or v.tick - mind.field_at > 600) and dist(v.me, mind.base) <= 4:
        compute_field(v)
        return
    if not mind.field:
        resupply(handle, v)
        return
    field = mind.field_set
    c, s = v.seen.crops, v.seen.soil
    crops = {(c[i], c[i + 1]): c[i + 3] for i in range(0, len(c), 4)}
    wheat = v.carried.get(Item.WHEAT, 0)
    seeds = v.carried.get(Item.WHEAT_SEED, 0)
    apple_seeds = v.carried.get(Item.APPLE_SEED, 0)
    apples = v.carried.get(Item.APPLE, 0)
    junk = v.carried.get(Item.WOOD, 0) + v.carried.get(Item.STONE, 0)
    room = CARRY - v.load
    ripe = [p for p, t in crops.items() if t <= v.tick and p in field] if room >= 6 else []
    plant = []
    if seeds or apple_seeds:
        soil = [(s[i], s[i + 1]) for i in range(0, len(s), 3) if apple_seeds or s[i + 2] >= 100]
        plant = [p for p in soil if p in field and p not in crops]
    produce = max(0, wheat - 3) + apples + max(0, seeds - 6)
    stocked = mind.stock_at < 0 or mind.stock.get(Item.WHEAT_SEED, 0) > 0
    want_apple = 2 if mind.stock_at < 0 or mind.stock.get(Item.APPLE_SEED) else 0
    unload = room < 6 or produce >= 12 or junk >= 10
    restock = not ripe and (seeds == 0 and stocked or apple_seeds == 0 and want_apple > 0)
    keep = {Item.WHEAT: 3, Item.WHEAT_SEED: 99, Item.APPLE_SEED: 2, Item.APPLE: 0}
    if base_full(v) or mind.stock.get(Item.APPLE, 0) >= 40:
        keep[Item.APPLE] = 99
    if base_full(v) or mind.stock.get(Item.WHEAT, 0) >= 150:
        keep[Item.WHEAT] = 99
    supplies = {Item.WHEAT_SEED: 6, Item.WHEAT: 3, Item.APPLE_SEED: want_apple}
    base_food = mind.stock.get(Item.WHEAT, 0) + mind.stock.get(Item.APPLE, 0)
    porter = (mind.id % 3 == 0 or (base_food < 10 and mind.id % 3 == 1)) and base_food < 30
    porter &= v.tick - mind.heard_at < 300
    apples_home = apples >= 4 and mind.stock.get(Item.APPLE, 0) < 40
    granary = not porter and not apples_home and junk == 0
    surplus = {Item.WHEAT: wheat - 3, Item.APPLE: apples, Item.WHEAT_SEED: seeds - 6}
    if unload and granary and fill_granary(handle, v, surplus):
        return
    if seeds > 12 and drop(handle, v, Item.WHEAT_SEED, seeds - 8):
        return
    if (unload or restock) and visit_base(handle, v, keep, supplies):
        return
    for item in (Item.WOOD, Item.STONE):
        if junk >= 10 and v.carried.get(item) and drop(handle, v, item, v.carried[item]):
            return
    blds = v.buildings()

    def open_ground(q: Pos) -> bool:
        return q in field and v.tiles[q] in (GRASS, DIRT) and q not in blds

    ripe_set, plant_set = set(ripe), set(plant)
    seed = Item.APPLE_SEED if apple_seeds else Item.WHEAT_SEED
    jobs: list[tuple[Callable[[Pos], bool], Callable[[Dir], object]]] = [
        (ripe_set.__contains__, handle.harvest),
        (plant_set.__contains__, lambda d: handle.plant(d, seed)),
        (open_ground, handle.till),
    ]
    for wants, act in jobs:
        for d, (dx, dy) in DIRS:
            if wants((v.me[0] + dx, v.me[1] + dy)):
                with suppress(ActionError):
                    act(d)
                    mind.path = []
                    return
    for key, targets in (("ripe", ripe), ("plant", plant)):
        if targets and go_beside(handle, v, targets, key):
            return
    till, clear = [], []
    for p in mind.field:
        if v.inside(p) and p not in blds:
            t = v.data[(p[1] - v.oy) * v.w + p[0] - v.ox]
            if t in (GRASS, DIRT):
                till.append(p)
            elif t in (FOREST, ROCK):
                clear.append(p)
    if till and go_beside(handle, v, till, "till"):
        return
    if not seeds and stocked:
        resupply(handle, v, {Item.WHEAT_SEED: 6})
        return
    if produce >= 4 and granary and fill_granary(handle, v, surplus):
        return
    for d, (dx, dy) in DIRS:
        q = (v.me[0] + dx, v.me[1] + dy)
        if q in field and v.tiles[q] in (FOREST, ROCK) and q not in blds:
            with suppress(ActionError):
                handle.gather(d)
                return
    if clear and go_beside(handle, v, clear, "clear"):
        return
    soon = next((p for p in crops if p in field), None)
    if soon and dist(v.me, soon) > 3:
        go_beside(handle, v, [soon], "wait")


# Building


def builder(handle: FrontierHandle, v: View) -> None:
    known(handle, v, 150)
    blds = v.buildings()
    working: list[tuple[Building, Pos]] = []
    fresh: list[tuple[Building, Pos]] = []
    repair: list[tuple[Building, Pos]] = []
    saving: Costs = {}
    if Building.MONUMENT in mind.costs and not v.own(MONUMENT, False):
        saving = {Item.WOOD: 60, Item.STONE: 60}
    piles = v.piles()
    have = plus(mind.stock, v.carried)
    sealed = at(0, 2) if at(*POST) not in v.mates else None
    for kind, p in mind.plan:
        b = blds.get(p)
        if b is not None:
            team, k, hp, left = b
            if team == v.team and BUILDINGS[k] == kind:
                if left > 0 and mind.skip.get(p, 0) <= v.tick:
                    working.append((kind, p))
                elif left == 0 and hp < mind.hp.get(kind, 0) * 0.6:
                    repair.append((kind, p))
            continue
        cost = mind.costs.get(kind)
        monument = kind == Building.MONUMENT
        if cost is None or (len(fresh) >= 6 and not monument):
            continue
        if mind.skip.get(p, 0) > v.tick or not v.inside(p) or p in piles or p == sealed:
            continue
        if v.tiles[p] not in (GRASS, DIRT, FOREST):
            continue
        if monument:
            if all(mind.stock.get(i, 0) >= n for i, n in cost.items()):
                fresh.insert(0, (kind, p))
            continue
        if all(have.get(i, 0) - saving.get(i, 0) >= n for i, n in cost.items()):
            fresh.append((kind, p))
    if working:
        kind, p = min(working, key=lambda w: dist(w[1], v.me))
        finish(handle, v, kind, p)
        return
    blocked = [p for _, p in mind.plan if p in piles and dist(p, mind.base) <= 6]
    if blocked and v.load < CARRY - 4 and pick_up(handle, v, blocked):
        return
    if fresh:
        if fresh[0][0] == Building.MONUMENT:
            mind.target = fresh[0]
        elif mind.target not in fresh:
            mind.target = fresh[mind.id % min(2, len(fresh))]
        kind, p = mind.target
        place(handle, v, kind, p, sum(1 for k, _ in fresh if k == kind))
        return
    if repair and v.tick - mind.stock_at < 200:
        kind, p = repair[0]
        d = dir_to(v.me, p)
        if d is not None:
            with suppress(ActionError):
                handle.build(d, kind)
                return
        go_beside(handle, v, [p], ("fix", p))
        return
    if v.tick - mind.stock_at > 60 or v.load > food_want(BUILDER) + 1:
        resupply(handle, v)
        return
    if not help_monument(handle, v):
        idle(handle, v)


def pick_up(handle: FrontierHandle, v: View, spots: list[Pos]) -> bool:
    """Takes what lies on one of `spots`, in the way of building, to bring to the base."""
    for p in spots:
        d = dir_to(v.me, p)
        if d is not None:
            item = max(v.piles()[p], key=lambda i: v.piles()[p][i])
            with suppress(ActionError):
                handle.take(d, item, CARRY - v.load)
                return True
    return go_beside(handle, v, spots, "pick-up")


def place(handle: FrontierHandle, v: View, kind: Building, p: Pos, count: int) -> None:
    """Places `kind` at `p`, fetching the cost of up to `count` of them in one trip."""
    load, room = {}, CARRY
    for item, n in mind.costs[kind].items():
        load[item] = min(n, room)
        room -= load[item]
    short = any(v.carried.get(i, 0) < n for i, n in load.items())
    per = max(1, sum(load.values()))
    more = {i: n * max(1, min(count, (CARRY - 4) // per)) for i, n in load.items()}
    if kind == Building.MONUMENT:
        more = load
    food = sum(load.values()) <= CARRY - food_want(BUILDER)
    if (short or base_store(v) is not None) and resupply(handle, v, more, food):
        return
    if short:
        mind.skip[p] = v.tick + 30
        return
    d = dir_to(v.me, p)
    if d is None:
        if not go_beside(handle, v, [p], ("place", p)):
            mind.skip[p] = v.tick + 40
        return
    try:
        handle.build(d, kind)
    except ActionError:
        mind.skip[p] = v.tick + 40


def finish(handle: FrontierHandle, v: View, kind: Building, p: Pos) -> None:
    d = dir_to(v.me, p)
    need = mind.owed.get(p, {})
    if d is None:
        if need and not any(v.carried.get(i) for i in need):
            if not resupply(handle, v, {i: min(n, CARRY) for i, n in need.items()}):
                mind.owed.pop(p, None)
        elif not go_beside(handle, v, [p], ("work", p)):
            wiggle(handle)
        return
    owed = next((o for q, _, _, o, _ in stores_beside(v) if q == p), {})
    mind.owed[p] = owed
    for item, n in owed.items():
        if v.carried.get(item):
            with suppress(ActionError):
                handle.put(d, item, n)
                return
    if owed:
        if any(not mind.stock.get(i) and not v.carried.get(i) for i in owed):
            mind.skip[p] = v.tick + 150
        resupply(handle, v, {i: min(n, CARRY) for i, n in owed.items()})
        return
    with suppress(ActionError):
        handle.build(d, kind)


def help_monument(handle: FrontierHandle, v: View) -> bool:
    sites = v.own(MONUMENT, finished=False)
    if not sites:
        return False
    p = sites[0]
    d = dir_to(v.me, p)
    if d is None:
        return go_beside(handle, v, [p], "monument")
    with suppress(ActionError):
        handle.build(d, Building.MONUMENT)
        return True
    return False


# Industry


def wanted(kind: Building) -> list[Recipe]:
    """What a workbench or smelter should make next, most wanted first, counting what the
    smith carries as still in the base."""
    s = plus(mind.stock, mind.carrying).get
    copper = s(Item.COPPER_INGOT, 0)
    if kind == Building.SMELTER:
        out = [Recipe.COPPER_INGOT, Recipe.COPPER_INGOT_CHARCOAL] if copper < 40 else []
        out.append(Recipe.IRON_INGOT)
        if s(Item.COPPER_ORE, 0) >= 2 and s(Item.CHARCOAL, 0) < 8 and s(Item.WOOD, 0) >= 150:
            out.append(Recipe.CHARCOAL)
        return out
    out = []
    if not s(Item.STONE_PICK) and mind.made.get(Recipe.STONE_PICK, 0) < 4:
        out.append(Recipe.STONE_PICK)
    if not s(Item.COPPER_PICK) and mind.made.get(Recipe.COPPER_PICK, 0) < 3:
        out.append(Recipe.COPPER_PICK)
    if s(Item.GEAR, 0) < 20:
        out.append(Recipe.GEAR)
    if copper >= 45 and mind.made.get(Recipe.COPPER_SWORD, 0) < 6:
        out.append(Recipe.COPPER_SWORD)
    if copper >= 45 and mind.made.get(Recipe.COPPER_ARMOUR, 0) < 6:
        out.append(Recipe.COPPER_ARMOUR)
    return out


def inputs(recipe: Recipe | None) -> Costs:
    return mind.recipes[recipe][1] if recipe in mind.recipes else {}


def recipe_for(kind: Building, avail: Costs) -> Recipe | None:
    for recipe in wanted(kind):
        need = inputs(recipe)
        if need and all(avail.get(i, 0) >= n for i, n in need.items()):
            return recipe
    return None


def smith(handle: FrontierHandle, v: View) -> None:
    """Studies what the team has found, and keeps the workbench and smelters busy."""
    known(handle, v, 200)
    infer_studied()
    mind.carrying = v.carried
    desks = v.own(DESK)
    for item in STUDY:
        if desks and item not in mind.studied and (mind.stock.get(item) or v.carried.get(item)):
            if study(handle, v, desks[0], item):
                return
            break
    blds = v.buildings()
    shops = {
        p: BUILDINGS[k]
        for p, (team, k, _, left) in blds.items()
        if team == v.team and left == 0 and BUILDINGS[k] in SHOPS
    }
    for p, d, stored, _, done in stores_beside(v):
        if p in shops:
            mind.shop[p] = (stored, done)
            if work_shop(handle, v, p, d, shops[p], stored, done >= 0):
                return
    plan = next_shop(v, shops)
    need = inputs(plan[1]) if plan else {}
    goods = sum(v.carried.get(i, 0) for i in GOODS if i not in need)
    if goods and (plan is None or CARRY - v.load < 6):
        resupply(handle, v, {i: v.carried.get(i, 0) for i in need})
        return
    if plan is None:
        if v.tick - mind.stock_at > 60 or v.load > 4:
            resupply(handle, v)
        elif not help_monument(handle, v):
            idle(handle, v)
        return
    p, _, ready = plan
    if not ready:
        room = CARRY - food_want(SMITH) - goods - 2
        have = plus(mind.stock, v.carried)
        runs = min([6, room // sum(need.values())] + [have.get(i, 0) // n for i, n in need.items()])
        resupply(handle, v, {i: n * max(runs, 1) for i, n in need.items()})
        return
    if not go_beside(handle, v, [p], ("shop", p)):
        stored, _ = mind.shop.get(p, ({}, -1))
        mind.shop[p] = (stored, v.tick + 20)


def work_shop(
    handle: FrontierHandle, v: View, p: Pos, d: Dir, kind: Building, stored: Costs, busy: bool
) -> bool:
    recipe = None if busy else recipe_for(kind, plus(stored, v.carried))
    need = inputs(recipe)
    room = CARRY - v.load > 0
    for item, n in stored.items():
        if room and item in mind.made_at.get(kind, ()) and item not in need:
            with suppress(ActionError):
                handle.take(d, item, n)
                return True
    if busy:
        return False
    limits = mind.limits.get(kind, {})
    for item, n in stored.items():
        if room and n > limits.get(item, 0):
            with suppress(ActionError):
                handle.take(d, item, n - limits.get(item, 0))
                return True
    if recipe is None:
        return False
    if all(stored.get(i, 0) >= n for i, n in need.items()):
        try:
            done = handle.craft(d, recipe)
        except ActionError:
            return False
        mind.shop[p] = (stored, done)
        mind.made[recipe] = mind.made.get(recipe, 0) + 1
        return True
    for item in need:
        if v.carried.get(item):
            with suppress(ActionError):
                handle.put(d, item, v.carried[item])
                return True
    return False


def next_shop(v: View, shops: dict[Pos, Building]) -> tuple[Pos, Recipe | None, bool] | None:
    """The nearest idle shop that can start a job now with what it holds and what is
    carried, or has goods to collect; or else one that could from the base's stock."""
    later = None
    for p in sorted(shops, key=lambda p: dist(p, v.me)):
        stored, done = mind.shop.get(p, ({}, -1))
        if done > v.tick:
            continue
        recipe = recipe_for(shops[p], plus(stored, v.carried))
        if recipe is not None:
            return (p, recipe, True)
        if later is None:
            recipe = recipe_for(shops[p], plus(stored, v.carried, mind.stock))
            if recipe is not None:
                later = (p, recipe, False)
        if any(i in mind.made_at.get(shops[p], ()) for i in stored):
            return (p, None, True)
    return later


def infer_studied() -> None:
    k, r = mind.costs, mind.recipes
    if Building.SMELTER in k:
        mind.studied.add(Item.STONE)
    if Building.TOWER in k:
        mind.studied |= {Item.STONE, Item.WOOD}
    if Recipe.COPPER_INGOT in r:
        mind.studied |= {Item.COAL, Item.COPPER_ORE}
    if Recipe.COPPER_INGOT_CHARCOAL in r:
        mind.studied |= {Item.CHARCOAL, Item.COPPER_ORE}
    if Recipe.IRON_INGOT in r:
        mind.studied |= {Item.COAL, Item.IRON_ORE}
    if Recipe.COPPER_PICK in r:
        mind.studied.add(Item.COPPER_INGOT)
    if Recipe.GEAR in r:
        mind.studied.add(Item.IRON_INGOT)
    if Building.STEAM_DRILL in k:
        mind.studied.add(Item.GEAR)
    if Building.MONUMENT in k:
        mind.studied.add(Item.APPLE)


def study(handle: FrontierHandle, v: View, desk: Pos, item: Item) -> bool:
    """Fetches `item` and spends it at the desk; False if it can't get there."""
    if not v.carried.get(item):
        return resupply(handle, v, {item: 1})
    d = dir_to(v.me, desk)
    if d is None:
        return go_beside(handle, v, [desk], "desk")
    with suppress(ActionError):
        handle.study(d, item)
        mind.studied.add(item)
        mind.known_at = -(10**9)
    return True


# Mining


def miner(handle: FrontierHandle, v: View) -> None:
    tile = MINES[mind.role]
    tool = TOOLS.get(mind.role)
    if tool and not v.carried.get(tool):
        fetch = mind.stock_at < 0 or mind.stock.get(tool) or v.tick - mind.stock_at > 300
        if fetch and resupply(handle, v, {tool: 1}):
            return
        tile = COAL
    item = ORE[tile]
    if v.carried.get(item) and (capped(v, item) or base_full(v)):
        if not stash(handle, v) and not drop(handle, v, item, v.carried[item]):
            wiggle(handle)
        return
    if tile != IRON and drill(handle, v, tile):
        return
    if mind.stock.get(item, 0) >= ORE_STOCK[tile] and not v.carried.get(item):
        gather(handle, v, FOREST, Item.WOOD, True)
        return
    if CARRY - v.load <= 0:
        if v.carried.get(item, 0) >= 8 or not dig(handle, v, tile):
            resupply(handle, v)
        return
    if harvest(handle, v, ("vein", tile), resource_goal(v, tile, 0)):
        mind.tries = 0
        return
    if v.carried.get(item, 0) >= 10:
        resupply(handle, v)
        return
    if dist(v.me, mind.base) > 20 and dig(handle, v, tile):
        return
    vein = mind.veins.get(tile)
    if vein is not None:
        if dist(v.me, vein) <= 6:
            mind.tries += 1
        if dist(v.me, vein) <= 1 or mind.tries > 20:
            mind.bad.add(vein)
            mind.veins.pop(tile, None)
            mind.tries = 0
        elif go_to(handle, v, vein, "vein", 1):
            return
        else:
            mind.veins.pop(tile, None)
    explore(handle, v)


def dig(handle: FrontierHandle, v: View, tile: int) -> bool:
    """Tunnels through rock toward a vein in view that no walk reaches; the stone dug is
    left on the ground."""
    goal = resource_goal(v, tile, 0)
    if not any(goal):
        return False
    path = bfs(v, goal, True, table=DIGGABLE)
    rocks = [k for k, p in enumerate(path or []) if v.tiles[p] == ROCK]
    if not path or not rocks:
        return False
    k = rocks[-1]
    rock = path[k]
    d = dir_to(v.me, rock)
    if d is None:
        mind.path, mind.path_key = path[k + 1 :], ("dig", rock)
        return step_path(handle, v)
    if CARRY - v.load <= 0:
        return drop(handle, v, Item.STONE, v.carried.get(Item.STONE, 0), tuple(WALKABLE))
    with suppress(ActionError):
        handle.gather(d)
        return True
    return False


def drill(handle: FrontierHandle, v: View, tile: int) -> bool:
    """Works a drill beside a vein three times as fast as a pick, or sets one up when the
    base has copper to spare."""
    if CARRY - v.load <= 0:
        return False
    for p in v.own(DRILL):
        d = dir_to(v.me, p)
        if d is not None and v.tiles[p] in (COAL, COPPER):
            with suppress(ActionError):
                handle.operate(d)
                return True
    for p in v.own(DRILL, finished=False):
        d = dir_to(v.me, p)
        if d is not None:
            with suppress(ActionError):
                handle.build(d, Building.MECH_DRILL)
                return True
    known(handle, v, 500)
    kit = mind.costs.get(Building.MECH_DRILL)
    if kit is None:
        return False
    if all(v.carried.get(i, 0) >= n for i, n in kit.items()):
        for d, (dx, dy) in DIRS:
            q = (v.me[0] + dx, v.me[1] + dy)
            if v.tiles[q] == tile and q not in v.buildings():
                with suppress(ActionError):
                    handle.build(d, Building.MECH_DRILL)
                    mind.drills += 1
                    return True
        return False
    spare = mind.stock.get(Item.COPPER_INGOT, 0) >= 44 and mind.stock.get(Item.WOOD, 0) >= 60
    if spare and mind.drills < 2 and base_store(v):
        return resupply(handle, v, kit)
    return False


ROLES: dict[int, Callable[[FrontierHandle, View], None]] = {
    WOODMAN: woodman,
    MASON: mason,
    FARMER: farmer,
    BUILDER: builder,
    SMITH: smith,
    GUARD: guard,
    COALER: miner,
    COPPERER: miner,
    IRONER: miner,
    HOARDER: hoarder,
}
