chunks = []


def step(handle):
    try:
        while True:
            chunks.append(bytearray(1 << 20))
    except MemoryError:
        print("MemoryError after", len(chunks), "MiB")
        chunks.clear()
