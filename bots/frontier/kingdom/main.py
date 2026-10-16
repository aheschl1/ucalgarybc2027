"""Kingdom: a walled town that farms, mines, smelts, and raises the Monument.

The commander never leaves its post beside the base: it spawns workers, gives each a role,
and relays what the team has found. The base sits in a ring of walls, gates, and towers;
spike traps cover the approaches and the farm. Workers farm wheat and apples by the pond,
cut wood and stone for the base, build the town from a fixed plan, study and craft at the
desk, workbench, and smelter, mine the veins the team finds, man the towers, and once the
base is stocked, hoard what they gather in chests in the wild, which counts for points.
Each unit runs its own copy of this file: module globals belong to one unit."""

import base64
from collections.abc import Callable, Iterable
from contextlib import suppress

from ucbc.games.frontier import (
    STEPS,
    Building,
    Dir,
    FrontierHandle,
    Item,
    Kind,
    Observation,
    Recipe,
    Store,
)
from ucbc.handle import ActionError

Pos = tuple[int, int]

CARRY = 20
GRASS, FOREST, ROCK, WATER, DIRT, COAL, COPPER, IRON, FARMLAND = range(9)
FREE = bytes(t in (GRASS, FOREST, DIRT, FARMLAND) for t in range(256))
MASK = [bytes(tile) + b"\x01" + bytes(255 - tile) for tile in range(9)]
WET = b"0" * WATER + b"1" + b"0" * (255 - WATER)
BUILDINGS = list(Building)
CROPS_WHEAT = 1
GATE = BUILDINGS.index(Building.GATE)
TRAP = BUILDINGS.index(Building.SPIKE_TRAP)
CHEST = BUILDINGS.index(Building.CHEST)
TOWER = BUILDINGS.index(Building.TOWER)
WELL = BUILDINGS.index(Building.WELL)
MONUMENT = BUILDINGS.index(Building.MONUMENT)
DIRS = list(STEPS.items())
FOOD = (Item.WHEAT, Item.APPLE)

WOODMAN, MASON, FARMER, BUILDER, SMITH, GUARD, COALER, COPPERER, IRONER, HOARDER = range(1, 11)
MINES = {COALER: COAL, COPPERER: COPPER, IRONER: IRON}
TOOLS = {COPPERER: Item.STONE_PICK, IRONER: Item.COPPER_PICK}
ORE = {COAL: Item.COAL, COPPER: Item.COPPER_ORE, IRON: Item.IRON_ORE}
EARLY = [WOODMAN, BUILDER, WOODMAN, BUILDER, MASON, WOODMAN, FARMER, GUARD, FARMER, SMITH, GUARD]

W, S = Item.WOOD, Item.STONE
COST: dict[Building, dict[Item, int]] = {
    Building.CHEST: {W: 10},
    Building.WOOD_WALL: {W: 4},
    Building.STONE_WALL: {S: 6},
    Building.RADIO_TOWER: {W: 20, S: 10},
    Building.WELL: {S: 10},
    Building.RESEARCH_DESK: {W: 15, S: 5},
    Building.WORKBENCH: {W: 10},
    Building.SMELTER: {S: 20},
    Building.GATE: {W: 6},
    Building.TOWER: {W: 5, S: 15},
    Building.SPIKE_TRAP: {W: 4, S: 2},
    Building.MONUMENT: {
        W: 60,
        S: 60,
        Item.COPPER_INGOT: 30,
        Item.IRON_INGOT: 30,
        Item.GEAR: 20,
        Item.APPLE: 30,
    },
}
HP = {
    Building.CHEST: 50,
    Building.WOOD_WALL: 100,
    Building.STONE_WALL: 300,
    Building.RADIO_TOWER: 80,
    Building.WELL: 100,
    Building.RESEARCH_DESK: 80,
    Building.WORKBENCH: 80,
    Building.SMELTER: 150,
    Building.GATE: 120,
    Building.TOWER: 250,
    Building.SPIKE_TRAP: 20,
    Building.MONUMENT: 2000,
}
UNKNOWN_AT_START = {Building.SMELTER, Building.TOWER, Building.SPIKE_TRAP, Building.MONUMENT}
STUDY = [
    Item.STONE,
    Item.WOOD,
    Item.COAL,
    Item.COPPER_ORE,
    Item.COPPER_INGOT,
    Item.IRON_ORE,
    Item.IRON_INGOT,
    Item.GEAR,
    Item.APPLE,
]
WOOD_STOCK, STONE_STOCK = 220, 150
ORE_STOCK = {COAL: 100, COPPER: 70, IRON: 80}
HOARD_FROM = 18
POP_CAP = 90
CHEST_ROOM = 40
APPLES = 31
DEBUG = False


def ring() -> list[tuple[Building, int, int]]:
    """The wall around the base, in the base's frame (x mirrored for the right-hand team),
    east (toward the other team) first. The pond lies outside the middle of the north side;
    the forest beyond the west side is cut from its gates."""
    wall, gate = Building.WOOD_WALL, Building.GATE
    east = [(gate, 4, 0)] + [(wall, 4, y) for y in (-4, -3, -2, -1, 1, 2, 3, 4)]
    north = [(gate, -3, -4), (gate, 3, -4)] + [(wall, x, -4) for x in (-4, -2, -1, 0, 1, 2)]
    south = [(gate, x, 4) for x in range(-2, 3)] + [(wall, -3, 4), (wall, -4, 4), (wall, 3, 4)]
    west = [(gate, -4, y) for y in range(-3, 4)]
    return east + north + south + west


def trap_spots() -> list[tuple[int, int]]:
    """Hidden spikes just outside the wall and around the farm, the east side first."""
    spots = []
    for d in (5, 6):
        spots += [(d, dy) for dy in range(-d, d + 1)]
        spots += [(dx, d) for dx in range(-d, d)]
        spots += [(dx, -d) for dx in range(-d, d + 1) if abs(dx) > 5]
    for d in (5, 6):
        spots += [(-d, dy) for dy in range(-d, d + 1)]
    return list(dict.fromkeys(spots))


GRANARIES = [(-2, -5), (2, -5), (-4, -12), (4, -12), (-5, -8), (5, -8)]
BROADCAST = [Item.WHEAT, Item.APPLE, Item.WOOD, Item.STONE]
TRAPS = trap_spots()
TRAP_SET = set(TRAPS)
POSTS = [((3, -1), (2, -1)), ((3, 1), (2, 1)), ((-3, -3), (-3, -2))]
WELLS = [(9, -9), (-9, -9), (0, -16)]
IDLE = [(-1, 2), (1, 2), (-1, -2), (1, -2), (1, 3)]


def town() -> list[tuple[Building, int, int]]:
    desk = [(Building.RESEARCH_DESK, -2, -2)]
    towers = [(Building.TOWER, x, y) for (x, y), _ in POSTS]
    shops = [(Building.WORKBENCH, 2, -2), (Building.SMELTER, 2, 2)]
    shops += [(Building.CHEST, x, y) for x, y in GRANARIES[:2]]
    later = [(Building.CHEST, x, y) for x, y in GRANARIES[2:]]
    later += [(Building.SMELTER, -2, 2), (Building.RADIO_TOWER, -3, 3)]
    later += [(Building.WELL, x, y) for x, y in WELLS]
    traps = [(Building.SPIKE_TRAP, dx, dy) for dx, dy in TRAPS]
    return desk + ring() + towers + shops + later + traps + [(Building.MONUMENT, -2, 1)]


class Rng:
    """A small xorshift generator; the random module is slow to import in the sandbox."""

    def __init__(self) -> None:
        self.state = 1

    def random(self) -> float:
        x = self.state
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        self.state = x
        return x / 4294967296

    def shuffle(self, items: list[Dir]) -> None:
        for i in range(len(items) - 1, 0, -1):
            j = int(self.random() * (i + 1))
            items[i], items[j] = items[j], items[i]


class Mind:
    def __init__(self) -> None:
        self.role = 0
        self.born = -1
        self.id = -1
        self.base: Pos = (0, 0)
        self.m = 1
        self.size = 0
        self.path: list[Pos] = []
        self.path_key: object = None
        self.stuck = 0
        self.fails: dict[object, int] = {}
        self.base_busy = -1
        self.empty: dict[Pos, int] = {}
        self.full_until: dict[Pos, int] = {}
        self.stock: dict[Item, int] = {}
        self.stock_at = -(10**6)
        self.base_full_until = 0
        self.heard_at = -(10**6)
        self.veins: dict[int, Pos] = {}
        self.explore: Pos | None = None
        self.explore_until = 0
        self.full: set[Pos] = set()
        self.bad: set[Pos] = set()
        self.tries = 0
        self.field: list[Pos] = []
        self.field_at = -(10**9)
        self.known: set[Building] = set()
        self.recipes: set[Recipe] = set()
        self.known_at = -(10**9)
        self.studied: set[Item] = set()
        self.shop: dict[Pos, tuple[dict[Item, int], int]] = {}
        self.made: dict[Recipe, int] = {}
        self.skip: dict[Pos, int] = {}
        self.plan: list[tuple[Building, Pos]] = []
        self.post: Pos | None = None
        self.alive: dict[int, tuple[int, int]] = {}
        self.assigned: list[tuple[int, int, int]] = []
        self.spawned = 0
        self.rng = Rng()


mind = Mind()


def at(dx: int, dy: int) -> Pos:
    return (mind.base[0] + mind.m * dx, mind.base[1] + dy)


def rel(p: Pos) -> Pos:
    return ((p[0] - mind.base[0]) * mind.m, p[1] - mind.base[1])


def dist(a: Pos, b: Pos) -> int:
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]))


class View:
    """One step's observation, decoded as far as the step needs."""

    def __init__(self, seen: Observation) -> None:
        self.seen = seen
        self.tick = seen.tick
        self.ox, self.oy = seen.origin.x, seen.origin.y
        self.w, self.h = seen.width, seen.height
        self.data = base64.b64decode(seen.tiles)
        self.me: Pos = (seen.me.pos.x, seen.me.pos.y)
        self.team = seen.me.team
        self.carried = {s.item: s.count for s in seen.me.carrying}
        self.load = sum(self.carried.values())
        self.enemies: list[tuple[Pos, int, int]] = []
        self.occupied: set[Pos] = set()
        r = seen.units
        for i in range(0, len(r), 6):
            p = (r[i + 3], r[i + 4])
            self.occupied.add(p)
            if r[i + 1] != self.team:
                self.enemies.append((p, r[i + 5], r[i + 2]))
        self._blds: dict[Pos, tuple[int, int, int, int]] | None = None
        self._piles: set[Pos] | None = None

    def tile(self, p: Pos) -> int:
        x, y = p[0] - self.ox, p[1] - self.oy
        if 0 <= x < self.w and 0 <= y < self.h:
            return self.data[y * self.w + x]
        return -1

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

    def piles(self) -> set[Pos]:
        if self._piles is None:
            r = self.seen.piles
            self._piles = {(r[i], r[i + 1]) for i in range(0, len(r), 4)}
        return self._piles

    def own(self, kind: int, finished: bool = True) -> list[Pos]:
        return [
            p
            for p, (team, k, _, left) in self.buildings().items()
            if team == self.team and k == kind and (left == 0) == finished
        ]

    def food(self) -> int:
        return self.carried.get(Item.WHEAT, 0) + self.carried.get(Item.APPLE, 0)


# Walking


def grid(v: View) -> bytearray:
    free = bytearray(v.data.translate(FREE))
    w, ox, oy = v.w, v.ox, v.oy
    for x, y in v.occupied:
        free[(y - oy) * w + x - ox] = 0
    b = v.seen.buildings
    for i in range(0, len(b), 7):
        if not (b[i + 1] == v.team and b[i + 6] == 0 and b[i + 2] in (GATE, TRAP)):
            free[(b[i + 4] - oy) * w + b[i + 3] - ox] = 0
    return free


def bfs(
    v: View, goal: bytes | bytearray, near: bool, reach: int = 0, toward: Pos | None = None
) -> list[Pos] | None:
    """A shortest walk to a tile in `goal`, or beside one when `near`, within `reach` of
    here if given; failing that, to the reachable tile nearest `toward` if given. The
    next step is last."""
    free = grid(v)
    w, n = v.w, len(free)
    mx, my = v.me[0] - v.ox, v.me[1] - v.oy
    if reach:
        x0, x1 = max(mx - reach, 0), min(mx + reach + 1, w)
        y0, y1 = max(my - reach, 0), min(my + reach + 1, v.h)
        free[: y0 * w] = bytes(y0 * w)
        free[y1 * w :] = bytes(n - y1 * w)
        for y in range(y0, y1):
            free[y * w : y * w + x0] = bytes(x0)
            free[y * w + x1 : (y + 1) * w] = bytes(w - x1)
    start = my * w + mx
    prev = [-1] * n
    prev[start] = start
    queue = [start]
    end = -1
    for i in queue:
        x = i % w
        if near:
            hit = (
                (x and goal[i - 1])
                or (x + 1 < w and goal[i + 1])
                or (i >= w and goal[i - w])
                or (i + w < n and goal[i + w])
            )
        else:
            hit = goal[i]
        if hit:
            end = i
            break
        if x and free[i - 1] and prev[i - 1] < 0:
            prev[i - 1] = i
            queue.append(i - 1)
        if x + 1 < w and free[i + 1] and prev[i + 1] < 0:
            prev[i + 1] = i
            queue.append(i + 1)
        j = i - w
        if j >= 0 and free[j] and prev[j] < 0:
            prev[j] = i
            queue.append(j)
        j = i + w
        if j < n and free[j] and prev[j] < 0:
            prev[j] = i
            queue.append(j)
    if end < 0:
        if toward is None:
            return None
        tx, ty = toward[0] - v.ox, toward[1] - v.oy
        end = min(queue, key=lambda i: max(abs(i % w - tx), abs(i // w - ty)))
    path = []
    while end != start:
        path.append((end % w + v.ox, end // w + v.oy))
        end = prev[end]
    return path


def step_path(handle: FrontierHandle, v: View) -> bool:
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
    if d is not None and nx in v.occupied and mind.stuck < 2:
        mind.stuck += 1
        return True
    mind.path = []
    mind.stuck += 1
    return False


def go(
    handle: FrontierHandle,
    v: View,
    key: object,
    goal: Callable[[], bytearray],
    near: bool,
    reach: int = 0,
    toward: Pos | None = None,
) -> bool:
    """Follows the walk for `key`, or plans one; True if it moved. A walk that can't be
    planned isn't tried again for a few ticks."""
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
        if path is None and reach:
            path = bfs(v, g, near, 0, toward)
    if not path:
        mind.path = []
        mind.fails[key] = v.tick + (3 if path is None else 6)
        if len(mind.fails) > 64:
            mind.fails = {k: t for k, t in mind.fails.items() if t > v.tick}
        return False
    mind.path = path
    return step_path(handle, v)


def wiggle(handle: FrontierHandle) -> bool:
    dirs = [d for d, _ in DIRS]
    mind.rng.shuffle(dirs)
    for d in dirs:
        with suppress(ActionError):
            handle.move(d)
            return True
    return False


def marks(v: View, targets: Iterable[Pos]) -> bytearray:
    goal = bytearray(v.w * v.h)
    for x, y in targets:
        x, y = x - v.ox, y - v.oy
        if 0 <= x < v.w and 0 <= y < v.h:
            goal[y * v.w + x] = 1
    return goal


def go_beside(handle: FrontierHandle, v: View, targets: list[Pos], key: object) -> bool:
    if not targets:
        return False
    near = min(dist(t, v.me) for t in targets)
    return go(handle, v, key, lambda: marks(v, targets), True, near + 5 if near < 10 else 0)


def go_to(handle: FrontierHandle, v: View, target: Pos, key: object, radius: int = 0) -> bool:
    """Toward a tile anywhere: through the view to it, to the view's edge on its side, or
    as close as the view allows."""
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

    if go(handle, v, (key, target), goal, False, 0, target):
        return True
    return towards(handle, v.me, target)


def towards(handle: FrontierHandle, me: Pos, target: Pos) -> bool:
    dx, dy = target[0] - me[0], target[1] - me[1]
    order = sorted(DIRS, key=lambda s: -(s[1][0] * dx + s[1][1] * dy) + mind.rng.random())
    for d, _ in order[:3]:
        with suppress(ActionError):
            handle.move(d)
            return True
    return False


def dir_to(me: Pos, p: Pos) -> Dir | None:
    for d, (dx, dy) in DIRS:
        if (me[0] + dx, me[1] + dy) == p:
            return d
    return None


# Messages: the commander says b"C" and records; workers say b"H", their role, and veins.


def encode(data: bytes) -> str:
    return base64.b64encode(data).decode()


def vein_records() -> bytes:
    out = b""
    for tile, (x, y) in mind.veins.items():
        out += b"V" + bytes([tile]) + x.to_bytes(2, "little") + y.to_bytes(2, "little")
    return out


def learn_vein(tile: int, p: Pos) -> None:
    if p in mind.bad:
        return
    old = mind.veins.get(tile)
    if old is None or dist(p, mind.base) < dist(old, mind.base):
        mind.veins[tile] = p


def read(data: bytes, sender: int, v: View) -> None:
    i = 1
    while i < len(data):
        tag = data[i : i + 1]
        if tag == b"R":
            uid = int.from_bytes(data[i + 1 : i + 4], "little")
            if uid == mind.id:
                if mind.role != data[i + 4]:
                    mind.path = []
                mind.role = data[i + 4]
            i += 5
        elif tag == b"S":
            for k, item in enumerate(BROADCAST):
                mind.stock[item] = int.from_bytes(data[i + 1 + 2 * k : i + 3 + 2 * k], "little")
            mind.heard_at = v.tick
            i += 1 + 2 * len(BROADCAST)
        elif tag == b"V":
            tile = data[i + 1]
            p = (
                int.from_bytes(data[i + 2 : i + 4], "little"),
                int.from_bytes(data[i + 4 : i + 6], "little"),
            )
            learn_vein(tile, p)
            i += 6
        else:
            return


def hear(v: View, commander: bool) -> None:
    for msg in v.seen.heard:
        data = base64.b64decode(msg.data)
        if commander and data[:1] == b"H" and len(data) >= 2:
            mind.alive[msg.sender] = (data[1], v.tick)
            read(data[1:], msg.sender, v)
        elif not commander and data[:1] == b"C":
            read(data, msg.sender, v)


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


# The base


def base_store(v: View) -> Store | None:
    for s in v.seen.stores:
        if (s.pos.x, s.pos.y) == mind.base:
            return s
    return None


def visit_base(
    handle: FrontierHandle, v: View, keep: dict[Item, int], want: dict[Item, int]
) -> bool:
    """Beside the base, unloads all but `keep` and takes up to `want`; elsewhere heads
    there. False once there's nothing left to do there."""
    store = base_store(v)
    if store is None:
        if dist(v.me, mind.base) <= 1 and v.seen.base is not None:
            return wiggle(handle)
        return go_home(handle, v)
    stored = {s.item: s.count for s in store.stored}
    mind.stock = stored
    mind.stock_at = v.tick
    d = dir_to(v.me, mind.base)
    assert d is not None
    mind.base_busy = v.tick
    for item, n in v.carried.items():
        extra = n - keep.get(item, 0)
        if extra > 0:
            try:
                handle.put(d, item, extra)
                return True
            except ActionError:
                mind.base_full_until = v.tick + 150
    room = CARRY - v.load
    for item, n in want.items():
        short = min(n - v.carried.get(item, 0), room)
        if item == Item.WHEAT and not stored.get(Item.WHEAT):
            item = Item.APPLE
            spare = stored.get(Item.APPLE, 0) - (APPLES if v.seen.me.hungry_at > v.tick else 0)
            short = min(short, spare)
        if short > 0 and stored.get(item):
            with suppress(ActionError):
                handle.take(d, item, short)
                return True
    mind.base_busy = -1
    return False


def go_home(handle: FrontierHandle, v: View) -> bool:
    post = at(0, 1)
    sides = [
        p for p in ((mind.base[0] + dx, mind.base[1] + dy) for _, (dx, dy) in DIRS) if p != post
    ]
    if dist(v.me, mind.base) <= 14:
        if go(handle, v, "home", lambda: marks(v, sides), False):
            return True
        return wiggle(handle) if dist(v.me, mind.base) <= 3 else towards(handle, v.me, mind.base)
    return go_to(handle, v, mind.base, "far-home", 2)


def idle(handle: FrontierHandle, v: View) -> None:
    """Out of the way, inside the wall."""
    spot = at(*IDLE[mind.id % len(IDLE)])
    if v.me != spot:
        go(handle, v, ("idle", spot), lambda: marks(v, [spot]), False)


def eat(handle: FrontierHandle, v: View) -> bool:
    """Food from a granary by the farm with wheat in it, or else from the base."""
    want = food_want(mind.role)
    granaries = [at(*g) for g in GRANARIES]
    for s in v.seen.stores:
        p = (s.pos.x, s.pos.y)
        if p in granaries and not s.owed:
            stored = {st.item: st.count for st in s.stored}
            d = dir_to(v.me, p) or Dir.UP
            if stored.get(Item.WHEAT) and v.food() < want:
                with suppress(ActionError):
                    handle.take(d, Item.WHEAT, want - v.food())
                    return True
            mind.empty[p] = v.tick + 150
    built = [g for g in granaries if g in v.own(CHEST)] if v.inside(granaries[0]) else granaries
    open_ = [g for g in built if mind.empty.get(g, 0) < v.tick]
    outside = dist(v.me, mind.base) > 4
    if outside and open_:
        g = min(open_, key=lambda g: dist(g, v.me))
        if go_beside(handle, v, [g], ("granary", g)):
            return True
        mind.empty[g] = v.tick + 30
    base_food = mind.stock.get(Item.WHEAT, 0) + mind.stock.get(Item.APPLE, 0)
    if outside and built and base_food < 3 and v.tick - mind.heard_at < 50:
        g = min(built, key=lambda g: dist(g, v.me))
        return dist(g, v.me) <= 2 or go_beside(handle, v, [g], ("granary", g))
    keep = {Item.WHEAT: 0, Item.APPLE: 0}
    keep.update(keeps(v))
    return visit_base(handle, v, keep, {Item.WHEAT: want})


def take_seeds(handle: FrontierHandle, v: View) -> bool:
    """Surplus seed out of a granary, for a hoarder to stash in the wild."""
    granaries = {at(*g) for g in GRANARIES}
    for s in v.seen.stores:
        p = (s.pos.x, s.pos.y)
        seeds = sum(st.count for st in s.stored if st.item == Item.WHEAT_SEED)
        spare = min(seeds, CARRY - v.load - 8)
        if p in granaries and spare > 0:
            with suppress(ActionError):
                handle.take(dir_to(v.me, p) or Dir.UP, Item.WHEAT_SEED, spare)
                return True
    return False


def fill_granary(handle: FrontierHandle, v: View, items: dict[Item, int]) -> bool:
    granaries = [at(*g) for g in GRANARIES]
    for s in v.seen.stores:
        p = (s.pos.x, s.pos.y)
        if p in granaries and not s.owed:
            held = sum(st.count for st in s.stored)
            if held < CHEST_ROOM:
                for item, n in items.items():
                    if n > 0 and (item == Item.WHEAT or held < CHEST_ROOM - 10):
                        with suppress(ActionError):
                            handle.put(dir_to(v.me, p) or Dir.UP, item, n)
                            mind.empty.pop(p, None)
                            return True
            mind.full_until[p] = v.tick + 100
    room = [g for g in granaries if mind.full_until.get(g, 0) < v.tick and g in v.own(CHEST)]
    if room:
        g = min(room, key=lambda g: dist(g, v.me))
        if go_beside(handle, v, [g], ("granary", g)):
            return True
        mind.full_until[g] = v.tick + 30
    return False


def needs_food(v: View) -> bool:
    left = v.seen.me.hungry_at - v.tick
    left += 100 * v.carried.get(Item.WHEAT, 0) + 150 * v.carried.get(Item.APPLE, 0)
    return left < 1.3 * dist(v.me, mind.base) + 60


def food_want(role: int) -> int:
    return 3 if role in (FARMER, BUILDER, SMITH, GUARD) else 5


def attack_adjacent(handle: FrontierHandle, v: View) -> bool:
    for (x, y), _, _ in sorted(v.enemies, key=lambda e: e[1]):
        if abs(x - v.me[0]) + abs(y - v.me[1]) == 1:
            with suppress(ActionError):
                handle.attack(x, y)
                return True
    return False


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
        mind.rng.state = seen.me.id * 7919 + 1
        mind.plan = [(k, at(dx, dy)) for k, dx, dy in town()]
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
            handle.say(encode(b"H" + bytes([mind.role]) + vein_records()))
    if (v.tick + mind.id) % 8 == 0:
        scan_veins(v)
    if attack_adjacent(handle, v):
        return
    if mind.role == HOARDER and v.seen.stores and v.food() and take_seeds(handle, v):
        return
    if needs_food(v):
        load = v.load - v.food()
        if mind.role == HOARDER and load >= 10 and dist(v.me, mind.base) > HOARD_FROM:
            stash(handle, v)
            return
        if eat(handle, v):
            return
    ROLES.get(mind.role, woodman)(handle, v)
    if dir_to(v.me, mind.base) is not None and mind.base_busy != v.tick:
        wiggle(handle)


def keeps(v: View) -> dict[Item, int]:
    keep = {Item.WHEAT: food_want(mind.role), Item.APPLE: food_want(mind.role)}
    tool = TOOLS.get(mind.role)
    if tool:
        keep[tool] = 1
    for weapon in (Item.COPPER_SWORD, Item.COPPER_ARMOUR, Item.IRON_SWORD, Item.IRON_ARMOUR):
        keep[weapon] = 1
    return keep


def resupply(
    handle: FrontierHandle, v: View, extra: dict[Item, int] | None = None, food: bool = True
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


# Commander


def commander(handle: FrontierHandle, v: View) -> None:
    hear(v, True)
    scan_veins(v)
    acted = attack_adjacent(handle, v)
    post = at(0, 1)
    if not acted and v.me != post and v.seen.base is not None:
        go_beside(handle, v, [mind.base], "post") if dist(v.me, mind.base) > 1 else None
    store = base_store(v)
    if store is not None and not acted:
        stored = {s.item: s.count for s in store.stored}
        mind.stock = stored
        d = dir_to(v.me, mind.base)
        assert d is not None
        food = v.food()
        if food < 3:
            item = Item.WHEAT if stored.get(Item.WHEAT) else Item.APPLE
            with suppress(ActionError):
                handle.take(d, item, 3 - food)
                acted = True
        if not acted:
            spawn(handle, v, stored)
    mind.alive = {k: a for k, a in mind.alive.items() if v.tick - a[1] < 1500}
    if DEBUG and v.tick % 100 == 0:
        roles = sorted(r for r, _ in mind.alive.values())
        print(v.tick, "stock", {i.value: n for i, n in mind.stock.items()}, "roles", roles)
    msg = b"C"
    for uid, role, t in mind.assigned:
        if v.tick - t < 6:
            msg += b"R" + uid.to_bytes(3, "little") + bytes([role])
    msg += b"S" + b"".join(
        min(mind.stock.get(i, 0), 65535).to_bytes(2, "little") for i in BROADCAST
    )
    msg += vein_records()
    with suppress(ActionError):
        handle.say(encode(msg[:64]))


def spawn(handle: FrontierHandle, v: View, stored: dict[Item, int]) -> None:
    """A worker while the farms can feed one more and the base can spare the wood."""
    pop = len(mind.alive)
    wood = stored.get(Item.WOOD, 0)
    food = stored.get(Item.WHEAT, 0) + stored.get(Item.APPLE, 0)
    reserve = 0 if mind.spawned < 6 else 25
    if wood < 10 + reserve or food < 10 + pop // 3:
        return
    farmland = len(v.seen.soil) // 3
    c = v.seen.crops
    trees = sum(1 for i in range(2, len(c), 4) if c[i] == 0)
    fed = 10 + int(farmland * 0.7) + 5 * trees
    gap = 2 if mind.spawned < len(EARLY) else 12
    if (mind.assigned and v.tick - mind.assigned[-1][2] < gap) or pop >= min(POP_CAP, fed):
        return
    role = pick_role(pop)
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
        FARMER: max(2, pop // 6 + 1),
        BUILDER: 2 if pop < 20 else 3,
        WOODMAN: 3 if pop < 20 else 4,
        MASON: 2 if pop < 30 else 3,
        SMITH: 1 if pop < 25 else 2,
        GUARD: 1 if pop < 12 else 3 if pop < 25 else 5,
        COALER: mining,
        COPPERER: mining,
        IRONER: mining,
    }
    short = max(target, key=lambda r: target[r] - counts.get(r, 0))
    return short if target[short] > counts.get(short, 0) else HOARDER


# Gathering and hoarding


def adjacent_tile(v: View, tile: int) -> Dir | None:
    blds = v.buildings()
    for d, (dx, dy) in DIRS:
        p = (v.me[0] + dx, v.me[1] + dy)
        if v.tile(p) == tile and p not in blds:
            return d
    return None


def resource_goal(v: View, tile: int, away: int) -> bytearray:
    """The view's tiles of `tile`, but none within `away` of the base."""
    goal = bytearray(v.data.translate(MASK[tile]))
    for p in v.buildings():
        x, y = p[0] - v.ox, p[1] - v.oy
        goal[y * v.w + x] = 0
    if away:
        bx, by = mind.base[0] - v.ox, mind.base[1] - v.oy
        x0, x1 = max(bx - away, 0), min(bx + away + 1, v.w)
        for y in range(max(by - away, 0), min(by + away + 1, v.h)):
            if x0 < x1:
                goal[y * v.w + x0 : y * v.w + x1] = bytes(x1 - x0)
    return goal


def gather(handle: FrontierHandle, v: View, tile: int, item: Item, hoard: bool) -> None:
    room = CARRY - v.load
    if room <= 0 or (room < 2 and v.carried.get(item, 0) > 0):
        if hoard or mind.base_full_until > v.tick:
            stash(handle, v)
        else:
            resupply(handle, v)
        return
    d = adjacent_tile(v, tile)
    away = HOARD_FROM if hoard else 0
    if d is not None and (not hoard or dist(v.me, mind.base) > away):
        with suppress(ActionError):
            handle.gather(d)
            mind.path = []
            return
    if go(handle, v, ("res", tile, away), lambda: resource_goal(v, tile, away), True):
        return
    explore(handle, v)


def explore(handle: FrontierHandle, v: View) -> None:
    if mind.explore is None or v.tick > mind.explore_until or dist(v.me, mind.explore) < 3:
        r = 30 + mind.rng.random() * min(160, mind.size / 4)
        x = int(mind.base[0] + (2 * mind.rng.random() - 1) * r)
        y = int(mind.base[1] + (2 * mind.rng.random() - 1) * r)
        mid = mind.size // 2
        x = min(x, mid - 12) if mind.m == 1 else max(x, mid + 12)
        mind.explore = (min(max(x, 1), mind.size - 2), min(max(y, 1), mind.size - 2))
        mind.explore_until = v.tick + int(r * 2) + 50
    go_to(handle, v, mind.explore, "explore", 2)


def stash(handle: FrontierHandle, v: View) -> None:
    """Into a chest nearby with room, a chest site, or a new chest; else toward wilder land."""
    granaries = {at(*g) for g in GRANARIES}
    for s in v.seen.stores:
        p = (s.pos.x, s.pos.y)
        b = v.buildings().get(p)
        if b is None or b[1] != CHEST or p in granaries:
            continue
        if s.owed:
            for st in s.owed:
                if v.carried.get(st.item):
                    with suppress(ActionError):
                        handle.put(dir_to(v.me, p) or Dir.UP, st.item, st.count)
                        return
            continue
        if sum(st.count for st in s.stored) >= CHEST_ROOM:
            mind.full.add(p)
            continue
        for item, n in v.carried.items():
            if item in FOOD or n <= 0:
                continue
            with suppress(ActionError):
                handle.put(dir_to(v.me, p) or Dir.UP, item, n)
                return
    sites = v.own(CHEST, finished=False)
    for p in sites:
        d = dir_to(v.me, p)
        if d is not None:
            with suppress(ActionError):
                handle.build(d, Building.CHEST)
                return
    chests = [p for p in v.own(CHEST) if p not in mind.full]
    if sites or chests:
        near = min(sites + chests, key=lambda p: dist(p, v.me))
        if dist(near, v.me) <= 8 and go_beside(handle, v, [near], ("chest", near)):
            return
    if v.carried.get(Item.WOOD, 0) >= 10 and dist(v.me, mind.base) > HOARD_FROM:
        piles = v.piles()
        for d, (dx, dy) in DIRS:
            p = (v.me[0] + dx, v.me[1] + dy)
            if (
                (p[0] + 2 * p[1]) % 5 == 0
                and v.tile(p) in (GRASS, DIRT)
                and p not in v.occupied
                and p not in v.buildings()
                and p not in piles
            ):
                with suppress(ActionError):
                    handle.build(d, Building.CHEST)
                    return
        spots = [
            (x, y)
            for y in range(v.me[1] - 4, v.me[1] + 5)
            for x in range(v.me[0] - 4, v.me[0] + 5)
            if (x + 2 * y) % 5 == 0 and v.tile((x, y)) in (GRASS, DIRT) and (x, y) not in piles
        ]
        spots = [p for p in spots if p not in v.buildings()]
        if not go_beside(handle, v, spots, "chest-spot"):
            wiggle(handle)
        return
    if dist(v.me, mind.base) <= HOARD_FROM:
        resupply(handle, v)
        return
    if v.carried.get(Item.WOOD, 0) < 10:
        resupply(handle, v)
        return
    wiggle(handle)


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


def woodman(handle: FrontierHandle, v: View) -> None:
    hoard = mind.stock_at >= 0 and mind.stock.get(Item.WOOD, 0) >= WOOD_STOCK
    if v.load and not hoard and v.carried.get(Item.WOOD, 0) == 0 and CARRY - v.load < 3:
        resupply(handle, v)
        return
    if not hoard and CARRY - v.load > 1:
        for d, (dx, dy) in DIRS:
            x, y = v.me[0] + dx, v.me[1] + dy
            zone = -14 <= y - mind.base[1] <= -5 and abs(x - mind.base[0]) <= 8
            if zone and v.tile((x, y)) == FOREST:
                with suppress(ActionError):
                    handle.gather(d)
                    return
        if go(handle, v, "farm-forest", lambda: farm_forest(v), True):
            return
    gather(handle, v, FOREST, Item.WOOD, hoard)


def mason(handle: FrontierHandle, v: View) -> None:
    if mind.stock_at >= 0 and mind.stock.get(Item.STONE, 0) >= STONE_STOCK:
        woodman(handle, v)
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
    tilled = (GRASS, DIRT, FARMLAND, FOREST)
    skip = TRAP_SET | set(GRANARIES) | set(WELLS)
    field = []
    for dy in range(-16, 14):
        for dx in range(-14, 15):
            if max(abs(dx), abs(dy)) < 5 or (dx, dy) in skip:
                continue
            p = at(dx, dy)
            x, y = p[0] - v.ox, p[1] - v.oy
            if 0 <= x < w and 0 <= y < h and wet[y] >> x & 1 and v.data[y * w + x] in tilled:
                field.append(p)
    mind.field = field
    mind.field_at = v.tick


def farmer(handle: FrontierHandle, v: View) -> None:
    if (not mind.field or v.tick - mind.field_at > 600) and dist(v.me, mind.base) <= 4:
        compute_field(v)
        return
    if not mind.field:
        resupply(handle, v)
        return
    c = v.seen.crops
    crops = {(c[i], c[i + 1]): (c[i + 2], c[i + 3]) for i in range(0, len(c), 4)}
    s = v.seen.soil
    soil = {(s[i], s[i + 1]): s[i + 2] for i in range(0, len(s), 3)}
    wheat = v.carried.get(Item.WHEAT, 0)
    seeds = v.carried.get(Item.WHEAT_SEED, 0)
    apple_seeds = v.carried.get(Item.APPLE_SEED, 0)
    room = CARRY - v.load
    field = mind.field
    ripe = [p for p in field if p in crops and crops[p][1] <= v.tick] if room >= 6 else []
    plant = []
    if seeds or apple_seeds:
        plant = [p for p in field if p in soil and p not in crops and soil[p] >= 100]
        if apple_seeds:
            plant = [p for p in field if p in soil and p not in crops]
    till = [p for p in field if v.tile(p) in (GRASS, DIRT) and p not in v.buildings()]
    clear = [p for p in field if v.tile(p) == FOREST and p not in v.buildings()]
    apples = v.carried.get(Item.APPLE, 0)
    wood = v.carried.get(Item.WOOD, 0)
    produce = max(0, wheat - 3) + apples + max(0, seeds - 6)
    stocked = mind.stock_at < 0 or mind.stock.get(Item.WHEAT_SEED, 0) > 0
    want_apple = 2 if mind.stock_at < 0 or mind.stock.get(Item.APPLE_SEED) else 0
    unload = room < 6 or produce >= 12 or wood >= 10
    restock = not ripe and seeds == 0 and stocked
    restock |= not ripe and apple_seeds == 0 and want_apple > 0
    keep = {Item.WHEAT: 3, Item.WHEAT_SEED: 6, Item.APPLE_SEED: 2, Item.APPLE: 0}
    supplies = {Item.WHEAT_SEED: 6, Item.WHEAT: 3, Item.APPLE_SEED: want_apple}
    base_food = mind.stock.get(Item.WHEAT, 0) + mind.stock.get(Item.APPLE, 0)
    porter = mind.id % 3 == 0 and base_food < 30 and v.tick - mind.heard_at < 300
    granary = not porter and apples < 4 and wood == 0
    surplus = {Item.WHEAT: wheat - 3, Item.WHEAT_SEED: seeds - 6}
    if unload and granary and fill_granary(handle, v, surplus):
        return
    if (unload or restock) and visit_base(handle, v, keep, supplies):
        return
    if DEBUG and v.tick % 10 == 0:
        print("farm", v.me, len(field), len(ripe), len(plant), len(till), unload, v.carried)
    for targets, act in (
        (ripe, lambda d: handle.harvest(d)),
        (plant, lambda d: handle.plant(d, Item.APPLE_SEED if apple_seeds else Item.WHEAT_SEED)),
        (till, lambda d: handle.till(d)),
    ):
        if not targets:
            continue
        tset = set(targets)
        for d, (dx, dy) in DIRS:
            if (v.me[0] + dx, v.me[1] + dy) in tset:
                with suppress(ActionError):
                    act(d)
                    mind.path = []
                    return
    for key, targets in (("ripe", ripe), ("plant", plant), ("till", till)):
        if targets and go_beside(handle, v, targets, key):
            return
    if not seeds and stocked:
        resupply(handle, v, {Item.WHEAT_SEED: 6})
        return
    if produce >= 4 and granary and fill_granary(handle, v, surplus):
        return
    cut = adjacent_tile(v, FOREST)
    if clear and cut is not None:
        dx, dy = STEPS[cut]
        if (v.me[0] + dx, v.me[1] + dy) in set(clear):
            with suppress(ActionError):
                handle.gather(cut)
                return
    if clear and go_beside(handle, v, clear, "clear"):
        return
    soon = [p for p in field if p in crops]
    if soon and dist(v.me, soon[0]) > 3:
        go_beside(handle, v, soon[:1], "wait")


# Building


def known(handle: FrontierHandle, v: View, every: int) -> None:
    if v.tick - mind.known_at < every:
        return
    k = handle.known()
    mind.known = {b.building for b in k.buildings}
    mind.recipes = {r.recipe for r in k.recipes}
    mind.known_at = v.tick


def builder(handle: FrontierHandle, v: View) -> None:
    known(handle, v, 150)
    blds = v.buildings()
    stone_ok = mind.stock.get(Item.STONE, 0) >= 12
    working: list[tuple[Building, Pos]] = []
    fresh: list[tuple[Building, Pos]] = []
    repair: list[tuple[Building, Pos]] = []
    for kind, p in mind.plan:
        b = blds.get(p)
        if b is not None:
            team, k, hp, left = b
            built = BUILDINGS[k]
            if team != v.team:
                continue
            if built == kind or (kind == Building.STONE_WALL and built == Building.WOOD_WALL):
                if left > 0:
                    working.append((built, p))
                elif hp < HP.get(built, 0) * 0.6:
                    repair.append((built, p))
            continue
        monument = kind == Building.MONUMENT
        if (len(fresh) >= 6 and not monument) or mind.skip.get(p, 0) > v.tick or not v.inside(p):
            continue
        ground = (GRASS, DIRT, FOREST) if kind == Building.CHEST else (GRASS, DIRT)
        if v.tile(p) not in ground or p in v.piles():
            continue
        if kind == Building.STONE_WALL and not stone_ok:
            kind = Building.WOOD_WALL
        if kind in UNKNOWN_AT_START and kind not in mind.known:
            continue
        if monument:
            if all(mind.stock.get(i, 0) >= n for i, n in COST[kind].items()):
                fresh.insert(0, (kind, p))
            continue
        if all(mind.stock.get(i, 0) + v.carried.get(i, 0) >= n for i, n in COST[kind].items()):
            fresh.append((kind, p))
    if DEBUG and v.tick % 25 == 0:
        print("build", v.me, working[:2], fresh[:3], repair[:1], v.carried, mind.stock_at)
    if working:
        kind, p = min(working, key=lambda w: dist(w[1], v.me))
        finish(handle, v, kind, p)
        return
    if fresh:
        kind, p = (
            fresh[0] if fresh[0][0] == Building.MONUMENT else fresh[mind.id % min(2, len(fresh))]
        )
        place(handle, v, kind, p, sum(1 for k, _ in fresh if k == kind))
        return
    if repair and v.tick - mind.stock_at < 200:
        kind, p = repair[0]
        if dir_to(v.me, p) is not None:
            with suppress(ActionError):
                handle.build(dir_to(v.me, p) or Dir.UP, kind)
                return
        go_beside(handle, v, [p], ("fix", p))
        return
    if v.tick - mind.stock_at > 60 or v.load > food_want(BUILDER) + 1:
        resupply(handle, v)
        return
    if not help_monument(handle, v):
        idle(handle, v)


def place(handle: FrontierHandle, v: View, kind: Building, p: Pos, count: int) -> None:
    """Places `kind` at `p`, fetching the cost of up to `count` of them in one trip."""
    cost = COST[kind]
    load = {i: min(n, CARRY) for i, n in cost.items()}
    if kind == Building.MONUMENT:
        load = {Item.WOOD: 20}
    short = any(v.carried.get(i, 0) < n for i, n in load.items())
    per = max(1, sum(load.values()))
    more = {i: n * max(1, min(count, (CARRY - 4) // per)) for i, n in load.items()}
    if kind == Building.MONUMENT:
        more = load
    food = sum(load.values()) <= CARRY - food_want(BUILDER)
    if (short or base_store(v) is not None) and resupply(handle, v, more, food):
        return
    if short:
        if not resupply(handle, v, load, food):
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
    if d is None:
        if not go_beside(handle, v, [p], ("work", p)):
            mind.skip[p] = v.tick + 20
        return
    site = next((s for s in v.seen.stores if (s.pos.x, s.pos.y) == p), None)
    owed = {s.item: s.count for s in site.owed} if site else {}
    for item, n in owed.items():
        if v.carried.get(item):
            with suppress(ActionError):
                handle.put(d, item, n)
                return
    if owed:
        resupply(handle, v, {i: min(n, CARRY) for i, n in owed.items()})
        return
    with suppress(ActionError):
        handle.build(d, kind)


# Industry

RECIPES: dict[Recipe, tuple[Building, dict[Item, int]]] = {
    Recipe.STONE_PICK: (Building.WORKBENCH, {Item.WOOD: 2, Item.STONE: 3}),
    Recipe.COPPER_PICK: (Building.WORKBENCH, {Item.WOOD: 2, Item.COPPER_INGOT: 3}),
    Recipe.GEAR: (Building.WORKBENCH, {Item.IRON_INGOT: 1}),
    Recipe.COPPER_SWORD: (Building.WORKBENCH, {Item.WOOD: 1, Item.COPPER_INGOT: 3}),
    Recipe.COPPER_ARMOUR: (Building.WORKBENCH, {Item.COPPER_INGOT: 5}),
    Recipe.COPPER_INGOT: (Building.SMELTER, {Item.COPPER_ORE: 2, Item.COAL: 1}),
    Recipe.IRON_INGOT: (Building.SMELTER, {Item.IRON_ORE: 2, Item.COAL: 2}),
}
GOODS = {
    Item.COPPER_INGOT,
    Item.IRON_INGOT,
    Item.GEAR,
    Item.STONE_PICK,
    Item.COPPER_PICK,
    Item.COPPER_SWORD,
    Item.COPPER_ARMOUR,
}
SHOPS = (BUILDINGS.index(Building.SMELTER), BUILDINGS.index(Building.WORKBENCH))
MADE = {
    SHOPS[0]: {Item.COPPER_INGOT, Item.IRON_INGOT},
    SHOPS[1]: GOODS - {Item.COPPER_INGOT, Item.IRON_INGOT},
}


def wanted(kind: int) -> list[Recipe]:
    """What a workbench or smelter should make next, most wanted first."""
    s = mind.stock.get
    copper = s(Item.COPPER_INGOT, 0)
    if BUILDINGS[kind] == Building.SMELTER:
        return [Recipe.COPPER_INGOT, Recipe.IRON_INGOT] if copper < 40 else [Recipe.IRON_INGOT]
    out = []
    if not s(Item.STONE_PICK) and mind.made.get(Recipe.STONE_PICK, 0) < 4:
        out.append(Recipe.STONE_PICK)
    if not s(Item.COPPER_PICK) and mind.made.get(Recipe.COPPER_PICK, 0) < 3:
        out.append(Recipe.COPPER_PICK)
    if s(Item.GEAR, 0) < 20:
        out.append(Recipe.GEAR)
    if copper >= 36 and mind.made.get(Recipe.COPPER_SWORD, 0) < 6:
        out.append(Recipe.COPPER_SWORD)
    if copper >= 36 and mind.made.get(Recipe.COPPER_ARMOUR, 0) < 6:
        out.append(Recipe.COPPER_ARMOUR)
    return out


def recipe_for(kind: int, avail: dict[Item, int]) -> Recipe | None:
    for recipe in wanted(kind):
        inputs = RECIPES[recipe][1]
        if recipe in mind.recipes and all(avail.get(i, 0) >= n for i, n in inputs.items()):
            return recipe
    return None


def plus(*parts: dict[Item, int]) -> dict[Item, int]:
    out: dict[Item, int] = {}
    for part in parts:
        for i, n in part.items():
            out[i] = out.get(i, 0) + n
    return out


def smith(handle: FrontierHandle, v: View) -> None:
    """Studies what the team has found, and keeps the workbench and smelters busy."""
    known(handle, v, 200)
    infer_studied()
    desks = v.own(BUILDINGS.index(Building.RESEARCH_DESK))
    for item in STUDY:
        if item not in mind.studied and desks and (mind.stock.get(item) or v.carried.get(item)):
            study(handle, v, desks[0], item)
            return
    blds = v.buildings()
    shops = {p: k for p, (team, k, _, left) in blds.items() if k in SHOPS and team == v.team}
    shops = {p: k for p, k in shops.items() if blds[p][3] == 0}
    for s in v.seen.stores:
        p = (s.pos.x, s.pos.y)
        if p in shops:
            stored = {st.item: st.count for st in s.stored}
            mind.shop[p] = (stored, s.job.done_at if s.job else -1)
            if work_shop(handle, v, p, shops[p], stored, s.job is not None):
                return
    goods = sum(v.carried.get(i, 0) for i in GOODS)
    plan = next_shop(v, shops)
    if DEBUG and v.tick % 20 == 0:
        print("smith", v.me, plan, shops, mind.shop, v.carried, mind.studied, mind.recipes)
    if goods and (plan is None or CARRY - v.load < 6):
        resupply(handle, v)
        return
    if plan is None:
        if v.tick - mind.stock_at > 60 or v.load > 4:
            resupply(handle, v)
        elif not help_monument(handle, v):
            idle(handle, v)
        return
    p, recipe, ready = plan
    if not ready:
        inputs = RECIPES[recipe][1]
        room = CARRY - food_want(SMITH) - goods - 2
        have = plus(mind.stock, v.carried)
        runs = min(
            [6, room // sum(inputs.values())] + [have.get(i, 0) // n for i, n in inputs.items()]
        )
        resupply(handle, v, {i: n * max(runs, 1) for i, n in inputs.items()})
        return
    go_beside(handle, v, [p], ("shop", p))


def work_shop(
    handle: FrontierHandle, v: View, p: Pos, kind: int, stored: dict[Item, int], busy: bool
) -> bool:
    d = dir_to(v.me, p)
    if d is None:
        return False
    for item, n in stored.items():
        if item in MADE[kind] and CARRY - v.load > 0:
            with suppress(ActionError):
                handle.take(d, item, n)
                return True
    if busy:
        return False
    for item, n in stored.items():
        extra = n - max(
            (6 * ins.get(item, 0) for b, ins in RECIPES.values() if BUILDINGS.index(b) == kind),
            default=0,
        )
        if extra > 0 and CARRY - v.load > 0:
            with suppress(ActionError):
                handle.take(d, item, extra)
                return True
    recipe = recipe_for(kind, plus(stored, v.carried))
    if recipe is None:
        return False
    inputs = RECIPES[recipe][1]
    if all(stored.get(i, 0) >= n for i, n in inputs.items()):
        try:
            done = handle.craft(d, recipe)
        except ActionError:
            return False
        mind.shop[p] = (stored, done)
        mind.made[recipe] = mind.made.get(recipe, 0) + 1
        return True
    for item in inputs:
        if v.carried.get(item):
            with suppress(ActionError):
                handle.put(d, item, v.carried.get(item, 0))
                return True
    return False


def next_shop(v: View, shops: dict[Pos, int]) -> tuple[Pos, Recipe, bool] | None:
    """The nearest idle shop that can start a job now with what it holds and what is
    carried, or else one that could from the base's stock."""
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
        if any(i in MADE[shops[p]] for i in stored):
            return (p, Recipe.GEAR, True)
    return later


def infer_studied() -> None:
    k, r = mind.known, mind.recipes
    if Building.SMELTER in k:
        mind.studied.add(Item.STONE)
    if Building.TOWER in k:
        mind.studied |= {Item.STONE, Item.WOOD}
    if Recipe.COPPER_INGOT in r:
        mind.studied |= {Item.COAL, Item.COPPER_ORE}
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


def study(handle: FrontierHandle, v: View, desk: Pos, item: Item) -> None:
    if not v.carried.get(item):
        resupply(handle, v, {item: 1})
        return
    d = dir_to(v.me, desk)
    if d is None:
        go_beside(handle, v, [desk], "desk")
        return
    with suppress(ActionError):
        handle.study(d, item)
    mind.studied.add(item)
    mind.known_at = -(10**9)


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
    if mind.stock.get(item, 0) >= ORE_STOCK[tile] and not v.carried.get(item):
        gather(handle, v, FOREST, Item.WOOD, True)
        return
    room = CARRY - v.load
    if room <= 0:
        resupply(handle, v)
        return
    d = adjacent_tile(v, tile)
    if d is not None:
        with suppress(ActionError):
            handle.gather(d)
            mind.path = []
            mind.tries = 0
            return
    if go(handle, v, ("vein", tile), lambda: resource_goal(v, tile, 0), True):
        return
    if v.carried.get(item, 0) >= 10:
        resupply(handle, v)
        return
    vein = mind.veins.get(tile)
    if vein is not None:
        if dist(v.me, vein) <= 6:
            mind.tries += 1
        if dist(v.me, vein) <= 1 or mind.tries > 20:
            mind.bad.add(vein)
            mind.veins.pop(tile, None)
            mind.tries = 0
        else:
            go_to(handle, v, vein, "vein", 1)
            return
    explore(handle, v)


# Guarding


def guard(handle: FrontierHandle, v: View) -> None:
    """Mans a tower inside the wall, or with no tower free, meets raiders at the wall."""
    for weapon in (Item.COPPER_SWORD, Item.COPPER_ARMOUR):
        fetch = mind.stock.get(weapon) and not v.carried.get(weapon)
        if fetch and dist(v.me, mind.base) < 8 and resupply(handle, v, {weapon: 1}):
            return
    towers = v.own(TOWER)
    near = sorted((e for e in v.enemies if dist(e[0], mind.base) <= 12), key=lambda e: e[1])
    for t in towers:
        if abs(t[0] - v.me[0]) + abs(t[1] - v.me[1]) == 1:
            for (x, y), _, _ in near:
                if dist((x, y), t) <= 6:
                    with suppress(ActionError):
                        handle.attack(x, y)
                        return
    posts = [at(*stand) for tower, stand in POSTS if at(*tower) in towers]
    if mind.post not in posts or (mind.post in v.occupied and mind.post != v.me):
        free = [p for p in posts if p not in v.occupied or p == v.me]
        mind.post = free[mind.id % len(free)] if free else None
    close = [e[0] for e in near if dist(e[0], mind.base) <= 7]
    if mind.post is None and close and v.seen.me.hp >= 50:
        target = min(close, key=lambda p: dist(p, v.me))
        go_beside(handle, v, [target], ("chase", target))
        return
    spot = mind.post or at(*IDLE[mind.id % len(IDLE)])
    if v.me != spot:
        if not go(handle, v, ("post", spot), lambda: marks(v, [spot]), False):
            help_monument(handle, v)
        return
    if v.tick - mind.stock_at > 400:
        resupply(handle, v)


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
    HOARDER: lambda handle, v: gather(handle, v, FOREST, Item.WOOD, True),
}
