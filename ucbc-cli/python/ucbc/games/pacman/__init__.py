"""Pac-Man as a bot sees it. A bot's ``step`` receives a :class:`PacmanHandle`: the
generated ``view`` and ``move`` plus a few conveniences for finding a way through the
maze."""

from collections import deque
from collections.abc import Callable, Iterable

from ucbc.games.pacman._api import AgentView, Coord, Dir, PacmanApi, Role, View

__all__ = ["HANDLE", "AgentView", "Coord", "Dir", "PacmanHandle", "Role", "View"]

STEPS = {Dir.UP: (0, -1), Dir.DOWN: (0, 1), Dir.LEFT: (-1, 0), Dir.RIGHT: (1, 0)}


class PacmanHandle(PacmanApi):
    @staticmethod
    def neighbour(view: View, at: Coord, dir: Dir) -> Coord:
        """The tile one step away, wrapping at the edges as the game does."""
        dx, dy = STEPS[dir]
        height, width = len(view.maze), len(view.maze[0])
        return Coord((at.x + dx) % width, (at.y + dy) % height)

    @staticmethod
    def can_enter(view: View, at: Coord, role: Role) -> bool:
        """Whether `role` may stand on `at`: never a wall, and Pac-Man never the house."""
        tile = view.maze[at.y][at.x]
        return tile != "#" and not (tile == "G" and role == Role.PACMAN)

    @classmethod
    def first_step(
        cls,
        view: View,
        start: Coord,
        role: Role,
        goal: Callable[[Coord], bool],
        avoid: Iterable[Coord] = (),
    ) -> Dir | None:
        """The first move of a shortest walk from `start` to a tile where `goal` holds,
        never through `avoid`. None if there's no such walk, or `start` is a goal."""
        blocked = set(avoid)
        first: dict[Coord, Dir | None] = {start: None}
        queue = deque([start])
        while queue:
            at = queue.popleft()
            if goal(at) and at != start:
                return first[at]
            for dir in Dir:
                nxt = cls.neighbour(view, at, dir)
                if nxt in first or nxt in blocked or not cls.can_enter(view, nxt, role):
                    continue
                first[nxt] = first[at] or dir
                queue.append(nxt)
        return None


HANDLE = PacmanHandle
