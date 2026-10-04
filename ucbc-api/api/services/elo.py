"""Elo ratings of participant teams. A new team starts at 800, the `teams.elo` default."""

K = 32


def expected(rating: float, opponent: float) -> float:
    return 1 / (1 + 10 ** ((opponent - rating) / 400))


def score(set_wins: list[int], sets: int) -> float:
    """Slot 0's score: its share of the sets, a drawn set worth half to each side."""
    return (sets + set_wins[0] - set_wins[1]) / (2 * sets)


def step(a: float, b: float, score_a: float) -> tuple[float, float]:
    """Both ratings after a match where `a` scored `score_a` and `b` the rest. What one
    gains the other loses."""
    delta = K * (score_a - expected(a, b))
    return a + delta, b - delta
