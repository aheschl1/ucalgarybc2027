from collections import deque

from ucbc.games.pacman import Coord, Dir, PacmanHandle, Role, View


def step(handle: PacmanHandle) -> None:
    view = handle.view()
    if view.me.role == Role.PACMAN:
        pacman_step(handle, view)
    else:
        ghost_step(handle, view)


# Pac-Man: heads for the nearest pellet, or frightened ghost, along a path that stays
# clear of the ghosts that can still catch it.


def pacman_step(handle: PacmanHandle, view: View) -> None:
    me = view.me.pos
    hunters = [g.pos for g in view.ghosts if not g.frightened]
    prey = {g.pos for g in view.ghosts if g.frightened}
    # A hunter's tile and every tile it could step to before we move again.
    danger = set(hunters)
    for at in hunters:
        danger.update(handle.neighbour(view, at, d) for d in Dir)

    def food(at: Coord) -> bool:
        return view.maze[at.y][at.x] in ".o" or at in prey

    dir = handle.first_step(view, me, Role.PACMAN, food, avoid=danger)
    if dir is None:
        # Cornered: any safe tile beats standing still.
        dir = handle.first_step(view, me, Role.PACMAN, lambda at: at not in danger)
    if dir is not None:
        handle.move(dir)


# Ghosts: chase Pac-Man along a shortest path, or run from it while frightened. Each
# ghost tries the directions in a different order, so ghosts with equally short paths
# split up instead of following each other.


def ghost_step(handle: PacmanHandle, view: View) -> None:
    me = view.me
    # Frightened ghosts move only on even ticks.
    if me.frightened and view.tick % 2:
        return
    dist = distances(handle, view, view.pacman.pos)
    k = me.id % 4
    dirs = list(Dir)[k:] + list(Dir)[:k]
    options = {d: dist[to] for d in dirs if (to := handle.neighbour(view, me.pos, d)) in dist}
    if not options:
        return
    pick = max if me.frightened else min
    handle.move(pick(options, key=options.__getitem__))


def distances(handle: PacmanHandle, view: View, start: Coord) -> dict[Coord, int]:
    """Steps from `start` to every tile a ghost can walk to."""
    dist = {start: 0}
    queue = deque([start])
    while queue:
        at = queue.popleft()
        for d in Dir:
            to = handle.neighbour(view, at, d)
            if to not in dist and handle.can_enter(view, to, Role.GHOST):
                dist[to] = dist[at] + 1
                queue.append(to)
    return dist
