from array import array
from pathlib import Path
from typing import Any

import pytest
from ucbc_rl import FrontierEnv

STARTER = Path(__file__).resolve().parents[2] / "bots" / "frontier" / "starter"
FLAT = '{"size": 64, "flat": true}'
SPAWN = 12


def numbers(kind: str, data: bytes) -> list[Any]:
    a = array(kind)
    a.frombytes(data)
    return a.tolist()


def actions(env: FrontierEnv, verb: int = 0, move: int = 0) -> bytes:
    row = [0] * len(env.heads)
    row[0], row[1] = move, verb
    return array("q", row * env.num_envs).tobytes()


def test_self_play_hands_out_a_unit_per_env_with_rewards_and_set_ends() -> None:
    env = FrontierEnv([None, None], game_config=FLAT, max_ticks=5)
    planes, rows, cols = env.grid_shape
    batch = env.reset()
    gone: list[tuple[Any, ...]] = []
    set_ends: list[tuple[Any, ...]] = []
    for _ in range(60):
        assert len(batch["grid"]) == env.num_envs * planes * rows * cols
        assert len(numbers("f", batch["vector"])) == env.num_envs * env.vector_len
        agents = numbers("q", batch["agent"])
        assert [agents[3 * i] for i in range(env.num_envs)] == list(range(env.num_envs))
        assert len(numbers("f", batch["reward"])) == 3 * env.num_envs
        gone += batch["gone"]
        set_ends += batch["set_ends"]
        batch = env.step(actions(env, SPAWN))
    assert {(e, ep) for e, ep, *_ in set_ends} >= {(0, 0), (1, 0)}
    winners = {(e, ep): w for e, ep, w, _ in set_ends}
    results = [r for e, ep, _, _, _, r in gone if (e, ep) in winners]
    assert results and all(r in (1.0, -1.0) for r in results)


def test_the_policy_plays_both_sides_against_a_python_team() -> None:
    env = FrontierEnv([STARTER, STARTER], game_config=FLAT, max_ticks=50)
    batch = env.reset()
    assert numbers("q", batch["team"]) == [0, 1]
    rejected: list[tuple[Any, ...]] = []
    for _ in range(20):
        batch = env.step(actions(env, SPAWN))
        rejected += batch["rejected"]
    # A commander's first spawn is taken; a worker's is refused.
    assert any(r == 0 for *_, r, _ in rejected) and any(r == 1 for *_, r, _ in rejected)


def test_both_teams_see_and_move_in_their_own_frame() -> None:
    env = FrontierEnv([None], game_config=FLAT, max_ticks=5)
    left = 3
    x = [numbers("f", env.reset()["vector"])[3]]
    for _ in range(3):
        x.append(numbers("f", env.step(actions(env, move=left))["vector"])[3])
    # Each commander steps left in its own frame: mirrored, the same tile further left.
    assert x[0] == x[1] and x[2] == x[3] == x[0] - 1 / 64


def test_bad_actions_and_teams_are_errors() -> None:
    env = FrontierEnv([None], game_config=FLAT, max_ticks=5)
    with pytest.raises(RuntimeError, match="step before reset"):
        env.step(actions(env))
    env.reset()
    with pytest.raises(RuntimeError, match="bytes of actions"):
        env.step(b"")
    bad = array("q", [5] + [0] * (len(env.heads) - 1)).tobytes()
    with pytest.raises(RuntimeError, match="head 0 id 5 is out of range"):
        env.step(bad)
    with pytest.raises(ValueError, match="no such file"):
        FrontierEnv(["/nonexistent"], game_config=FLAT)
