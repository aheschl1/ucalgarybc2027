import helpers
from pkg import grid


def step(handle):
    handle.memory["n"] = handle.memory.get("n", 0) + 1
    helpers.seen.append(handle.memory["n"])
    print(handle.memory["n"], helpers.seen, grid.size(), handle._query({"tick": handle.tick}))
