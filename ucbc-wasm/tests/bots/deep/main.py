def down(n):
    return down(n + 1)


def step(handle):
    if handle.tick == 0:
        try:
            down(0)
        except RecursionError:
            print("python recursion")
        return
    nested = []
    for _ in range(1_000_000):
        nested = [nested]
    repr(nested)
