import os
import time


def attempt(f):
    try:
        f()
        return "allowed"
    except OSError as e:
        return type(e).__name__


def write():
    with open("/bot/new.txt", "w") as f:
        f.write("x")


def read_outside():
    with open("/etc/hostname") as f:
        f.read()


def step(handle):
    start = time.perf_counter_ns()
    sum(range(1000))
    print("clock", time.perf_counter_ns() - start)
    print("random", os.urandom(4).hex())
    print("write", attempt(write))
    print("outside", attempt(read_outside))
    print("sleep", attempt(lambda: time.sleep(1)))
