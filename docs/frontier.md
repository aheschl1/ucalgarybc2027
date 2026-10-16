# Frontier (plan)

A 2-team survival-industry-war game on a large generated map, built as a long-horizon
competition and RL environment. Status: Phase 6 done, plus traps; next, the RL environment.

## Principles

- The game is rules and physics. Every decision is player code.
- Bots are units only. Buildings are passive; units act on them.
- A bot knows what it sees now. Map memory, pathfinding, planning, coordination are
  player code.
- Deterministic for a seed. Rules live in the game crate behind `Game`, with no
  dependence on the runner, so a host-side environment can step them later.

## World

- `game_config`: `size`, `units` (the cap), `vision`, `keyframe`, and `flat` (grass but
  for the starting areas, for learning on).
- Grid up to 4096 x 4096 (1024 x 1024 is the target), generated from the seed and
  mirrored left to right: grass, forest, rock, water, coal, copper and iron veins, and a
  titanium seam in the highest ground between the bases (4 tiles a side, 10 titanium
  each), which a team has to go out for. Each base gets a forest grove, rock,
  and a pond beside it. Resources are finite; a tile gathered out turns to dirt.
- Snapshots are deltas. `initial_state` carries the map; each tick carries changed
  units, buildings, tiles, ground items, and signs; a keyframe lists all of them every
  N ticks.

## Units

- One bot per unit. Each team starts with a **commander** and a **base**.
- The commander spawns units on tiles next to the base, standing next to it, paid from
  the base's storage. The base has HP; while it's destroyed there's no spawning. Only
  the commander can rebuild it.
- The commander is killed or starves: the team loses.
- Unit ability comes from equipment (tools, weapons, armour), not unit types.
- A unit eats from what it carries when its last meal runs out (an item's `food` is
  the ticks it feeds); with nothing to eat it loses 1 HP a tick and dies at 0. A
  starving commander loses the set. Hard population cap per team.

## Instruction set

Per step a unit may move once, take one other action, and `say` once. Directions are
the four neighbours.

```text
queries   observe                 tiles in vision as bytes; units, buildings, ground items as
                                  flat integer rows; signs; adjacent storage; what was heard
          known                   the team's recipes and buildings, in full
actions   move dir                one tile
          gather dir              one item from forest, rock, or a vein
          put | take dir item n   to/from a teammate, the team's storage, or the ground
          build dir building      place a site, paying the whole cost or a full load of it
                                  (the rest delivered with put); work on it; repair
          till | plant | harvest  farmland from grass or dirt; seeds near water; ripe crops
          craft dir recipe        start a known recipe at a building from what it stores
          operate dir             work a drill: its rate of items from the tile under it
          study dir item          spend an item at a research building, learning what it unlocks
          attack x y              hit a unit or building of the other team, or a crop, next
                                  to you or in range of your tower next to you
          spawn                   commander, next to the base, paid from its storage
          say bytes               to teammates in range or on the radio network, next tick
          mark bytes              sign on your tile, readable by anyone who sees it
later     -
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

- **Farming:** crops are data (`content/crops.toml`). Planting needs water within 4
  tiles, or a well. A crop stores the tick it ripens; anyone may harvest it. The yield
  scales with fertility, which a harvest wears and rest recovers (fertilizer from
  industry later). Wheat ripens fast and wears the soil; apple trees take 600 ticks,
  then regrow fruit forever.
- **Industry:** recipes are data (`content/recipes.toml`); fuel is an input like any
  other. The best tool a unit carries limits the veins it gathers: hands take coal,
  copper needs a stone pick, iron a copper pick, titanium an iron pick. A craft at a
  workbench or smelter runs as a batch from its storage. A drill stands on rock or a
  vein and digs its rate per `operate`; a steam drill burns coal and reaches iron and
  titanium. Smelters, drills, and most recipes start unknown: a team learns one once it
  has studied every item it uses, so knowledge follows the materials it has found.
- **War:** a hit does the attacker's best weapon (fists 3) less the target's best
  armour, at least 1. A killed unit drops what it carried; a destroyed building spills
  its storage. `take` reaches the other team's storage, so stealing is a walk there and
  back. Attacking a crop destroys it. A unit next to its team's tower reaches the
  tower's range and takes half damage. Gates let only their team through; walls come in
  material tiers; `build` repairs a damaged building a little (less than a sword hit). A destroyed base stops spawning
  until the commander rebuilds it. A finished trap is hidden from the other team and
  walkable; the first of them to step on it takes its damage, and it's spent.

## Winning

1. Kill the enemy commander, or outlast it: a starving commander loses too.
2. Finish the Monument: wood, stone, copper and iron ingots, gears, apples, and 20
   titanium, delivered to its site, then 400 work.
3. At the tick limit, the most points: what the team's finished buildings cost, what
   they store, and 5 a unit; then the most units; then a coin toss on the seed.

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

## Bots

The Python handle decodes the rows, and has `Walker`, `beside`, `nearest`, and `towards`
for finding walks through what a unit sees.

- `starter`: the economy. The commander spawns while the base has spare food; a third of
  the workers farm by the base's pond, the rest cut wood; all head home to unload and eat.
- `raider`: the starter, plus every fourth worker raiding: it packs food, walks to the
  mirror image of its own base, hits the units it meets, and steals food. On a 256 map
  it beats the starter 2-0, killing a commander; on 1024 a raid outlasts the CLI's
  1000-tick limit.

## Viewer

`games/frontier/viewer` rebuilds each frame from the deltas: entities from the nearest
keyframe, tiles from a change log over the initial map. The terrain is a 1-pixel-a-tile
canvas repainted where tiles change, scaled onto the board; markers keep a few screen
pixels at any zoom. Snapshots carry each team's points by source (buildings, stored,
units); the panel charts them over the set, per team and by source. Build it into the
viewer with `make build GAMES="ucbc2027 pacman frontier"`.

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
