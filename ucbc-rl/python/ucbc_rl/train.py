"""A small reference PPO trainer for `FrontierEnv`: one shared policy, every unit its own
trajectory. A CNN over the unit's grid and an MLP over its vector feed a GRU cell, then
one softmax per action head and a value head. The GRU state is kept per unit; an update
reruns one GRU step from each row's stored state rather than backpropagating through
time. A set cut off at `max_ticks` ends its units' trajectories like any other end.

    uv run --package ucbc-rl --extra train ucbc-rl-train --envs 64 --flat
    uv run --package ucbc-rl --extra train ucbc-rl-train --record runs --record-every 20
    uv run ucbc view runs/env0-20.json.gz
"""

import json
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import click
import numpy as np
import torch
from torch import Tensor, nn
from torch.nn import functional as F

from ucbc_rl import FrontierEnv

Agent = tuple[int, int, int]

# Planes of the grid (`ucbc_rl::frontier::plane`): categories to one-hot, or a scale.
CATEGORIES = {0: 11, 1: 5, 3: 16, 4: 5, 7: 5}
SCALES = {2: 100.0, 5: 255.0, 6: 255.0, 8: 101.0}

MOVE, VERB, DIR, ITEM, COUNT, BUILDING, RECIPE, TARGET, ORDER = range(9)
VERBS = ["none", "gather", "put", "take", "build", "till", "plant", "harvest", "craft"]
VERBS += ["operate", "study", "attack", "spawn"]
# The heads each verb reads, besides move and verb.
READS = {
    "gather": [DIR],
    "till": [DIR],
    "harvest": [DIR],
    "operate": [DIR],
    "put": [DIR, ITEM, COUNT],
    "take": [DIR, ITEM, COUNT],
    "build": [DIR, BUILDING],
    "plant": [DIR, ITEM],
    "craft": [DIR, RECIPE],
    "study": [DIR, ITEM],
    "attack": [TARGET],
}
USES = torch.zeros(len(VERBS), ORDER + 1, dtype=torch.bool)
USES[:, [MOVE, VERB]] = True
for _i, _verb in enumerate(VERBS):
    USES[_i, READS.get(_verb, [])] = True


def uses(actions: Tensor) -> Tensor:
    """[n, heads] bool: which heads count toward an action's probability."""
    used = USES.to(actions.device)[actions[:, VERB]]
    used[:, ORDER] = (actions[:, MOVE] > 0) & (actions[:, VERB] > 0)
    return used


class Residual(nn.Module):
    def __init__(self, width: int) -> None:
        super().__init__()
        self.a = nn.Conv2d(width, width, 3, padding=1)
        self.b = nn.Conv2d(width, width, 3, padding=1)

    def forward(self, x: Tensor) -> Tensor:
        out: Tensor = self.b(F.relu(self.a(F.relu(x))))
        return x + out


class Policy(nn.Module):
    """Residual convs over the grid at full resolution; a coarser map whose cells see each
    other through a transformer layer; the vector through an MLP of its own; a query from
    that and the unit's memory that attends over those cells; then the GRU and the heads. An attack's target scores each
    cell around the unit against its full-resolution features."""

    def __init__(
        self, env: FrontierEnv, hidden: int = 256, width: int = 64, status: int = 128
    ) -> None:
        super().__init__()
        planes = sum(CATEGORIES.values()) + len(SCALES)
        _, side, _ = env.grid_shape
        self.reach = int(env.heads[TARGET] ** 0.5)
        self.full = nn.Sequential(
            nn.Conv2d(planes, width, 3, padding=1), Residual(width), Residual(width)
        )
        self.coarse = nn.Sequential(
            nn.Conv2d(width, width, 3, stride=2, padding=1),
            nn.ReLU(),
            nn.Conv2d(width, width, 3, stride=2, padding=1),
            Residual(width),
            nn.Flatten(2),
        )
        cells = self.coarse(torch.zeros(1, width, side, side)).shape[2]
        self.position = nn.Parameter(torch.zeros(1, cells, width))
        self.mix = nn.TransformerEncoderLayer(width, 4, 2 * width, dropout=0.0, batch_first=True)
        self.status = nn.Sequential(
            nn.Linear(env.vector_len, status), nn.ReLU(), nn.Linear(status, status), nn.ReLU()
        )
        self.query = nn.Linear(status + hidden, width)
        self.attend = nn.MultiheadAttention(width, 4, batch_first=True)
        self.joint = nn.Sequential(nn.Linear(2 * width + status, hidden), nn.ReLU())
        self.gru = nn.GRUCell(hidden, hidden)
        self.heads = nn.ModuleList(
            nn.Linear(hidden, n) for i, n in enumerate(env.heads) if i != TARGET
        )
        self.point = nn.Linear(hidden, width)
        self.value = nn.Linear(hidden, 1)
        self.hidden = hidden

    def forward(
        self, grid: Tensor, vector: Tensor, h: Tensor
    ) -> tuple[list[Tensor], Tensor, Tensor]:
        planes = [
            F.one_hot(grid[:, p].long(), n).permute(0, 3, 1, 2) for p, n in CATEGORIES.items()
        ]
        planes += [grid[:, p : p + 1] / s for p, s in SCALES.items()]
        full = self.full(torch.cat(planes, 1).float())
        cells = self.mix(self.coarse(full).transpose(1, 2) + self.position)
        status = self.status(vector)
        query = self.query(torch.cat([status, h], 1)).unsqueeze(1)
        seen, _ = self.attend(query, cells, cells)
        x = self.joint(torch.cat([seen.squeeze(1), cells.mean(1), status], 1))
        h = self.gru(x, h)
        logits = [head(h) for head in self.heads]
        # The REACH x REACH cells centred on the unit, in the order the target head counts them.
        middle, r = full.shape[2] // 2, self.reach // 2
        around = full[:, :, middle - r : middle + r + 1, middle - r : middle + r + 1].flatten(2)
        target = torch.einsum("nw,nwc->nc", self.point(h), around) / around.shape[1] ** 0.5
        logits.insert(TARGET, target)
        return logits, self.value(h).squeeze(1), h


def judge(logits: list[Tensor], actions: Tensor) -> tuple[Tensor, Tensor]:
    """Log-probability and entropy of each action, over the heads it uses."""
    used = uses(actions).float()
    logps = [F.log_softmax(lg, 1) for lg in logits]
    logp = torch.stack(
        [lp.gather(1, actions[:, i : i + 1]).squeeze(1) for i, lp in enumerate(logps)], 1
    )
    entropy = torch.stack([-(lp.exp() * lp).sum(1) for lp in logps], 1)
    return (logp * used).sum(1), (entropy * used).sum(1)


def sample(logits: list[Tensor]) -> Tensor:
    return torch.stack([torch.multinomial(F.softmax(lg, 1), 1).squeeze(1) for lg in logits], 1)


@dataclass
class Turns:
    """One `FrontierEnv` batch as arrays, with the events beside it."""

    grid: Tensor
    vector: Tensor
    agents: list[Agent]
    rewards: np.ndarray[Any, np.dtype[np.float32]]
    raw: dict[str, Any]

    @classmethod
    def of(cls, env: FrontierEnv, raw: dict[str, Any]) -> "Turns":
        n = env.num_envs
        grid = np.frombuffer(raw["grid"], np.uint8).reshape(n, *env.grid_shape)
        vector = np.frombuffer(raw["vector"], np.float32).reshape(n, env.vector_len)
        agents = np.frombuffer(raw["agent"], np.int64).reshape(n, 3).tolist()
        return cls(
            torch.from_numpy(grid.copy()),
            torch.from_numpy(vector.copy()),
            [(e, ep, bot) for e, ep, bot in agents],
            np.frombuffer(raw["reward"], np.float32).reshape(n, 3),
            raw,
        )


@dataclass
class Config:
    envs: int = 64
    steps: int = 128
    iterations: int = 10000
    opponents: list[str | None] = field(default_factory=lambda: [None])
    game_config: dict[str, Any] = field(default_factory=lambda: {"size": 64})
    max_ticks: int = 1500
    alpha: float = 0.4
    scale: float = 100.0
    gamma: float = 0.999
    lam: float = 0.95
    lr: float = 3e-4
    epochs: int = 3
    minibatch: int = 4096
    clip: float = 0.2
    entropy: float = 0.01
    seed: int = 0
    out: Path | None = None
    # Each env writes every `record_every`th set it plays here, for `ucbc view`.
    record: Path | None = None
    record_every: int = 100

    def mix(self, r: Any) -> float:
        """One reward from the env's (own, team, result)."""
        own, team = r[0] / self.scale, r[1] / self.scale
        return float(self.alpha * own + (1 - self.alpha) * team + r[2])


KEYS = ("grid", "vector", "h", "act", "logp", "value")


@dataclass
class Rollout:
    """One window of turns: a row per unit-turn, its reward filled in when it arrives."""

    c: Config
    opponents: list[str | None]
    steps: list[tuple[Tensor, ...]] = field(default_factory=list)
    agent: list[Agent] = field(default_factory=list)
    reward: list[float] = field(default_factory=list)
    done: list[bool] = field(default_factory=list)
    # Each unit's newest row, until its reward arrives.
    pending: dict[Agent, int] = field(default_factory=dict)
    wins: int = 0
    sets: int = 0
    rejected: int = 0
    acted: int = 0

    def push(self, agents: list[Agent], *rows: Tensor) -> None:
        self.steps.append(rows)
        for a in agents:
            self.pending[a] = len(self.reward)
            self.agent.append(a)
            self.reward.append(0.0)
            self.done.append(False)

    def settle(self, t: Turns, memory: dict[Agent, Tensor]) -> None:
        for i, a in enumerate(t.agents):
            if a in self.pending:
                self.reward[self.pending.pop(a)] = self.c.mix(t.rewards[i])
        for e, ep, bot, *r in t.raw["gone"]:
            memory.pop((e, ep, bot), None)
            if (row := self.pending.pop((e, ep, bot), None)) is not None:
                self.reward[row], self.done[row] = self.c.mix(r), True
        for e, _, winner, _ in t.raw["set_ends"]:
            if self.opponents[e] is not None:
                self.sets += 1
                self.wins += winner == e % 2
        for *_, n, total in t.raw["rejected"]:
            self.rejected, self.acted = self.rejected + n, self.acted + total


def collect(
    env: FrontierEnv, model: Policy, memory: dict[Agent, Tensor], turns: Turns, r: Rollout
) -> tuple[Turns, dict[Agent, float]]:
    """Plays `steps` turns of every env into `r`. Returns the turns waiting after them,
    and the value of each of those units' state."""
    device = next(model.parameters()).device

    def forward(t: Turns) -> tuple[list[Tensor], Tensor, Tensor, Tensor]:
        h = torch.stack([memory.get(a, torch.zeros(model.hidden)) for a in t.agents])
        with torch.no_grad():
            logits, value, h_out = model(t.grid.to(device), t.vector.to(device), h.to(device))
        return logits, value.cpu(), h_out.cpu(), h

    for _ in range(r.c.steps):
        logits, value, h_out, h = forward(turns)
        act = sample(logits)
        logp, _ = judge(logits, act)
        for i, a in enumerate(turns.agents):
            memory[a] = h_out[i]
        r.push(turns.agents, turns.grid, turns.vector, h, act.cpu(), logp.cpu(), value)
        turns = Turns.of(env, env.step(act.cpu().numpy().astype(np.int64).tobytes()))
        r.settle(turns, memory)
    _, value, _, _ = forward(turns)
    return turns, dict(zip(turns.agents, value.tolist()))


def advantages(
    r: Rollout, values: list[float], bootstrap: dict[Agent, float]
) -> tuple[Tensor, Tensor]:
    """GAE per unit over its rows with a reward, and those rows. A unit's last row still
    waiting on one is left out, and its value stands in for what follows."""
    for a, row in r.pending.items():
        bootstrap[a] = values[row]
    waiting = set(r.pending.values())
    by_agent: dict[Agent, list[int]] = {}
    for row, a in enumerate(r.agent):
        if row not in waiting:
            by_agent.setdefault(a, []).append(row)
    adv = torch.zeros(len(r.reward))
    for a, rows in by_agent.items():
        # Only a unit's last row can end it.
        next_v = 0.0 if r.done[rows[-1]] else bootstrap[a]
        gae = 0.0
        for row in reversed(rows):
            gae = r.reward[row] + r.c.gamma * next_v - values[row] + r.c.gamma * r.c.lam * gae
            adv[row], next_v = gae, values[row]
    kept = sorted(row for rows in by_agent.values() for row in rows)
    return adv, torch.tensor(kept, dtype=torch.long)


def learn(
    model: Policy,
    optim: torch.optim.Optimizer,
    data: dict[str, Tensor],
    adv: Tensor,
    kept: Tensor,
    c: Config,
) -> dict[str, float]:
    """`c.epochs` passes of clipped PPO over the kept rows."""
    device = next(model.parameters()).device
    returns = adv + data["value"]
    gain = (adv - adv[kept].mean()) / (adv[kept].std() + 1e-8)
    stats = {"policy": 0.0, "value": 0.0, "entropy": 0.0}
    updates = 0
    for _ in range(c.epochs):
        order = kept[torch.randperm(len(kept))]
        for start in range(0, len(order), c.minibatch):
            mb = order[start : start + c.minibatch]
            grid, vector, h = (data[k][mb].to(device) for k in ("grid", "vector", "h"))
            logits, value, _ = model(grid, vector, h)
            logp, entropy = judge(logits, data["act"][mb].to(device))
            g = gain[mb].to(device)
            ratio = (logp - data["logp"][mb].to(device)).exp()
            policy = -torch.min(ratio * g, ratio.clamp(1 - c.clip, 1 + c.clip) * g).mean()
            value_loss = (value - returns[mb].to(device)).pow(2).mean()
            loss = policy + 0.5 * value_loss - c.entropy * entropy.mean()
            optim.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            optim.step()
            stats["policy"] += policy.item()
            stats["value"] += value_loss.item()
            stats["entropy"] += entropy.mean().item()
            updates += 1
    return {k: v / max(updates, 1) for k, v in stats.items()}


def train(c: Config) -> list[dict[str, float]]:
    """Runs `c.iterations` collect-then-learn rounds; returns each round's stats."""
    torch.manual_seed(c.seed)
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    opponents = [c.opponents[i % len(c.opponents)] for i in range(c.envs)]
    config = json.dumps(c.game_config)
    env = FrontierEnv(
        opponents,
        game_config=config,
        max_ticks=c.max_ticks,
        seed=c.seed,
        record_dir=c.record,
        record_every=c.record_every,
    )
    model = Policy(env).to(device)
    optim = torch.optim.Adam(model.parameters(), lr=c.lr)
    memory: dict[Agent, Tensor] = {}
    turns = Turns.of(env, env.reset())
    history = []
    for iteration in range(c.iterations):
        started = time.perf_counter()
        r = Rollout(c, opponents)
        turns, bootstrap = collect(env, model, memory, turns, r)
        collected = time.perf_counter()
        data = {k: torch.cat(v) for k, v in zip(KEYS, zip(*r.steps))}
        adv, kept = advantages(r, data["value"].tolist(), bootstrap)
        stats = learn(model, optim, data, adv, kept, c)
        stats |= {
            "iteration": iteration,
            "turns_per_s": c.envs * c.steps / (time.perf_counter() - started),
            "collect_s": collected - started,
            "learn_s": time.perf_counter() - collected,
            "reward": float(np.mean([r.reward[i] for i in kept.tolist()] or [0.0])),
            "rejected": r.rejected / max(r.acted, 1),
            "sets": r.sets,
            "win_rate": r.wins / max(r.sets, 1),
        }
        history.append(stats)
        click.echo(
            " ".join(
                f"{k}={v:.4g}" if isinstance(v, float) else f"{k}={v}" for k, v in stats.items()
            )
        )
        if c.out is not None:
            c.out.mkdir(parents=True, exist_ok=True)
            torch.save(model.state_dict(), c.out / "policy.pt")
    return history


@click.command()
@click.option("--envs", default=Config.envs, show_default=True)
@click.option("--steps", default=Config.steps, show_default=True, help="Turns per env per round.")
@click.option("--iterations", default=Config.iterations, show_default=True)
@click.option(
    "--opponent",
    "opponents",
    multiple=True,
    help="A Python team's directory, or 'self'; envs cycle through them. Default: self-play.",
)
@click.option("--size", default=64, show_default=True)
@click.option("--flat", is_flag=True, help="Grass but for the bases: a simple map.")
@click.option("--units", default=None, type=int, help="Each team's unit cap.")
@click.option("--max-ticks", default=Config.max_ticks, show_default=True)
@click.option("--seed", default=Config.seed, show_default=True)
@click.option("--out", type=click.Path(path_type=Path), help="Save the policy here each round.")
@click.option(
    "--record",
    type=click.Path(path_type=Path),
    help="Write replays here, as env<e>-<set>.json.gz, for `ucbc view`.",
)
@click.option(
    "--record-every",
    default=Config.record_every,
    show_default=True,
    help="Each env records every this many sets.",
)
def main(
    envs: int,
    steps: int,
    iterations: int,
    opponents: tuple[str, ...],
    size: int,
    flat: bool,
    units: int | None,
    max_ticks: int,
    seed: int,
    out: Path | None,
    record: Path | None,
    record_every: int,
) -> None:
    """Train a Frontier policy with PPO."""
    game_config: dict[str, Any] = {"size": size, "flat": flat}
    if units is not None:
        game_config["units"] = units
    train(
        Config(
            envs=envs,
            steps=steps,
            iterations=iterations,
            opponents=[None if o == "self" else o for o in opponents] or [None],
            game_config=game_config,
            max_ticks=max_ticks,
            seed=seed,
            out=out,
            record=record,
            record_every=record_every,
        )
    )
