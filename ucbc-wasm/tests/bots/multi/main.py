import helpers
from pkg import grid

n = 0


def step(handle):
    global n
    n += 1
    helpers.seen.append(n)
    print(n, helpers.seen, grid.size(), handle._query({"tick": handle.tick}))
