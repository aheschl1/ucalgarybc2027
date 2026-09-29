import random


def step(handle):
    total = 0
    for i in range(30_000):
        total += i * i
    print("finished", random.random())
