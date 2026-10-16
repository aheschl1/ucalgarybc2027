# Frontier (plan)

A 2-team survival-industry-war game on a large generated map, built as a long-horizon
competition and RL environment. Status: Phase 2.

## Principles

- The game is rules and physics. Every decision is player code.
- Bots are units only. Buildings are passive; units act on them.
- A bot knows what it sees now. Map memory, pathfinding, planning, coordination are
  player code.
- Deterministic for a seed. Rules live in the game crate behind `Game`, with no
  dependence on the runner, so a host-side environment can step them later.

## World

- Grid up to 4096 x 4096 (1024 x 1024 is the target), generated from the seed and
  mirrored left to right: grass, forest, rock, water, coal, copper and iron veins. Each
  base gets a forest grove and rock beside it. Resources are finite; a tile gathered
  out turns to dirt.
- Snapshots are deltas. `initial_state` carries the map; each tick carries changed
  units, buildings, tiles, ground items, and signs; a keyframe lists all of them every
  N ticks.

## Units

- One bot per unit. Each team starts with a **commander** and a **base**.
- The commander spawns units on tiles next to the base, standing next to it, paid from
  the base's storage. The base has HP; while it's destroyed there's no spawning. Only
  the commander can rebuild it.
- The commander dies: the team loses.
- Unit ability comes from equipment (tools, weapons, armour), not unit types.
- Food upkeep per unit; starvation lowers output, then HP. Hard population cap per team.

## Instruction set

Per step a unit may move once, take one other action, and `say` once. Directions are
the four neighbours.

```text
queries   observe                 tiles in vision as bytes; units, buildings, ground items as
                                  flat integer rows; signs; adjacent storage; what was heard
actions   move dir                one tile
          gather dir              one item from forest, rock, or a vein
          put | take dir item n   to/from a teammate, the team's storage, or the ground
          build dir building      place (paying the cost carried) or work on a site
          spawn                   commander, next to the base, paid from its storage
          say bytes               to teammates in range or on the radio network, next tick
          mark bytes              sign on your tile, readable by anyone who sees it
later     known, till | plant | harvest, use, attack
```

Ids for items, buildings, and recipes are generated enums in the SDK. Their details are
discovered in play: experimenting at a workshop, picking up, seeing or capturing enemy
buildings, ruins, research.

## Buildings

Data-driven: each building is a set of behaviours, each behaviour one Rust component.
New content is data; a new behaviour is one component.

```toml
[building.steam_drill]
upgrades = "mech_drill"
hp = 400
build = { iron_plate = 20, gear = 10 }
drill = { rate = 3, max_ore_tier = 3 }
fuel = { burns = ["coal", "charcoal"], per_tick = 0.1 }
storage = { slots = 4 }
```

Who has to be there:

| Thing | Needs a unit | Runs alone |
|---|---|---|
| Drill | Every step it mines: multiplies the operator's mining | |
| Tower | Every step it fires: gives the soldier range and cover | Empty: a wall |
| Smelter, workshop, kiln | To load inputs and fuel, choose the recipe | The batch, until inputs or fuel run out |
| Crops | To till, plant, water, harvest | Growth, fertility loss |
| Storage, walls, gates | To move items, open and close | |
| Radio tower | To build and fuel | Extends `say` range while fuelled |
| Base | Commander next to it, to spawn | |

A content test checks every input exists, every item is reachable from raw resources,
tiers increase along each `upgrades` line, and there are no cycles.

## Pillars

- **Farming:** till, plant, irrigate, harvest. Fertility falls with use and recovers by
  rotation or fertilizer from industry. Fast low-yield crops vs slow orchards that yield
  forever.
- **Industry:** tool tier limits ore tier. Ore, smelted with fuel, makes ingots; a
  workshop makes parts, tools, weapons, armour; drills tier from mechanical to steam to
  powered.
- **War:** attack, steal from storage or units and carry it home, burn crops, wreck
  buildings. Walls by material tier, manned towers, traps.

## Winning

1. Kill the enemy commander.
2. Complete the Monument: top-tier inputs from every line, built over many ticks.
3. At the tick limit, the higher score: assets, territory, food.

## Phase 0 results

1024² map, 2 teams, Python bots in the sandbox. The Phase 0 `walker` bot decoded its whole view and stepped
to a random free neighbour; units crowd around the bases, so most of the team is in view.
Steady state over the last 100 ticks.

| Units/team | Vision | Step median | Step p99 | Bot time/tick | Peak RSS | Replay/100 ticks |
|---|---|---|---|---|---|---|
| 10 | 16 | 136 µs | 146 µs | 2.7 ms | 257 MiB | 0.08 MiB |
| 50 | 16 | 421 µs | 462 µs | 40 ms | 1.2 GiB | 0.2 MiB |
| 100 | 16 | 627 µs | 834 µs | 121 ms | 2.4 GiB | 0.33 MiB |
| 200 | 16 | 947 µs | 1401 µs | 360 ms | 4.9 GiB | 0.56 MiB |
| 100 | 32 | 900 µs | 906 µs | 178 ms | 2.4 GiB | 0.32 MiB |
| 100 | 64 | 1054 µs | 1058 µs | 210 ms | 2.5 GiB | 0.31 MiB |

Where a step's time goes, 100 units/team: a step that only moves costs 18 µs; `observe`
in sparse surroundings about 120 µs; the rest scales with the units in view, decoded into
Python objects. Engine time outside bots is about 10 ms/tick at 200 bots. Memory is about
12 MiB per bot: the snapshot's pages are shared.

Consequences:

- Population cap 100 per team: about 120 ms/tick, so 10k ticks in about 20 minutes.
- `observe` sends units as flat integer arrays, decoded on demand by the handle, not as
  objects; the map window stays bytes.
- Snapshots send units as flat arrays too: replays are about 30 MiB per 10k ticks at 100
  per team before that.

## Phases

| | |
|---|---|
| 0 | Scaling prototype: 1024² generated map, 100-200 bots that move and observe, delta snapshots. Measure step time, tick time, match time, replay size, memory. |
| 1 | World and units: map generation, move, gather, carry, base, commander, spawn, vision, `say`, `mark`, content loader and test, generated ids |
| 2 | Farming and food upkeep |
| 3 | Industry, discovery, drills |
| 4 | War |
| 5 | Win conditions, starter bots |
| 6 | Canvas viewer from deltas |

Later: a host-side RL environment, and an LLM harness.
