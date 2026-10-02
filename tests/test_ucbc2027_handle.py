from typing import Any

import pytest
from ucbc.games.ucbc2027 import Coord, Direction, Ucbc2027Handle
from ucbc.handle import ActionError, Identity


def dino_at(x: int, y: int, blocked: set[tuple[int, int]]) -> tuple[Ucbc2027Handle, list[Coord]]:
    """A handle on a dino at (x, y) whose moves onto `blocked` are refused, and the
    moves it made."""
    pos = {"x": x, "y": y}
    moves: list[Coord] = []

    def bridge(name: str, payload: dict[str, Any]) -> dict[str, Any]:
        if payload["type"] == "me":
            return {"ok": {"type": "dino", "pos": pos, "level": 1, "health": 10, "held": None}}
        assert payload["type"] == "move"
        to = (payload["x"], payload["y"])
        if to in blocked:
            return {"err": {"kind": "action", "message": f"{to} is not free"}}
        moves.append(Coord(*to))
        return {"ok": {"x": to[0], "y": to[1]}}

    return Ucbc2027Handle(Identity(2, 0, "a", 0, "ucbc2027"), bridge), moves


def test_move_dir_steps_one_tile() -> None:
    handle, moves = dino_at(3, 3, set())
    assert handle.move_dir(Direction.NW) == Coord(2, 2)
    assert moves == [Coord(2, 2)]


def test_step_toward_goes_diagonal_then_falls_back() -> None:
    handle, moves = dino_at(3, 3, set())
    handle.step_toward(9, 0)
    assert moves == [Coord(4, 2)]
    handle, moves = dino_at(3, 3, {(4, 2)})
    handle.step_toward(9, 0)
    assert moves == [Coord(4, 3)]
    handle, moves = dino_at(3, 3, {(4, 3)})
    handle.step_toward(9, 3)
    assert moves == [Coord(4, 4)]


def test_step_toward_raises_the_first_refusal_and_stays_on_the_board() -> None:
    handle, moves = dino_at(0, 3, {(0, 2), (1, 2)})
    with pytest.raises(ActionError, match=r"\(0, 2\)"):
        handle.step_toward(0, 0)
    assert moves == []


def test_step_toward_its_own_tile_does_not_move() -> None:
    handle, moves = dino_at(3, 3, set())
    assert handle.step_toward(3, 3) == Coord(3, 3)
    assert moves == []


def test_a_lab_has_no_position() -> None:
    def bridge(name: str, payload: dict[str, Any]) -> dict[str, Any]:
        return {"ok": {"type": "lab", "origin": {"x": 1, "y": 7}, "health": 100}}

    handle = Ucbc2027Handle(Identity(0, 0, "a", 0, "ucbc2027"), bridge)
    with pytest.raises(TypeError):
        handle.pos()
