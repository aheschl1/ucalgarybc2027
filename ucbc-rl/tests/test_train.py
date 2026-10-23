import math
from pathlib import Path

import pytest

pytest.importorskip("torch")

from ucbc_rl.train import Config, train


def test_a_few_rounds_against_self_and_a_python_team_learn_without_errors() -> None:
    starter = str(Path(__file__).resolve().parents[2] / "bots" / "frontier" / "starter")
    history = train(
        Config(
            envs=4,
            steps=32,
            iterations=3,
            opponents=[None, starter],
            game_config={"size": 64, "flat": True},
            max_ticks=20,
            minibatch=64,
        )
    )
    assert len(history) == 3
    assert all(math.isfinite(h[k]) for h in history for k in ("policy", "value", "entropy"))
    assert sum(h["sets"] for h in history) > 0


def test_rewards_reach_the_turn_they_are_for_and_gae_sums_them() -> None:
    import numpy as np
    import torch
    from ucbc_rl.train import Rollout, Turns, advantages

    a, b = (0, 0, 1), (0, 0, 2)
    r = Rollout(Config(alpha=0.4, gamma=1.0, lam=1.0), [None])
    r.push([a, b])
    r.push([a])
    # A's next turn pays for its row 2 (own 100 points); B leaves with a win.
    raw = {"gone": [(*b, 0.0, 0.0, 1.0)], "set_ends": [], "rejected": []}
    rewards = np.array([[100.0, 0.0, 0.0]], dtype=np.float32)
    r.settle(Turns(torch.zeros(1), torch.zeros(1), [a], rewards, raw), {})
    assert r.reward == pytest.approx([0.0, 1.0, 0.4])
    assert r.done == [False, True, False]
    # A's newest row still waits on its reward: left out, its value carried back.
    r.push([a])
    adv, kept = advantages(r, [0.5, 0.2, 0.1, 0.3], {})
    assert kept.tolist() == [0, 1, 2]
    # Reward to go less the value: A's 0.4 then its 0.3 stand-in, B's 1 then nothing.
    assert adv[:3].tolist() == pytest.approx([0.0 + 0.4 + 0.3 - 0.5, 1.0 - 0.2, 0.4 + 0.3 - 0.1])
