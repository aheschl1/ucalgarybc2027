"""Places, then on its second turn spins until just short of the budget, short by more
in each set, so that in some set the step runs out of fuel after it reported `done`,
in the guest's cleanup. The bot clock is fuel, so which set that is never changes."""

import time

from ucbc.games.tictactoe import TicTacToeHandle

# The test's `step_ms`.
BUDGET_NS = 1_000_000
# Finer than that cleanup, about 1 us of bot time; the test's 120 sets cover 30 us, well
# past the ~11 us from here to the end of the call.
SHORT_BY_NS = 250

turn = 0


def step(handle: TicTacToeHandle) -> None:
    global turn
    start = time.perf_counter_ns()
    turn += 1
    handle.place(*handle.empty_cells()[0])
    if turn == 2:
        deadline = start + BUDGET_NS - handle.set_index * SHORT_BY_NS
        while time.perf_counter_ns() < deadline:
            pass
        print("spun")
