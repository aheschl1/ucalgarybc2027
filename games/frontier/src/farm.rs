//! Farming and food. A crop knows the tick it ripens, and soil the tick it began to rest,
//! so nothing about either changes from tick to tick.

use ucbc_engine::{ActionError, BotId, TeamId};

use crate::content::{Crop, Item};
use crate::map::{Coord, Dir, Tile};
use crate::rules::{CARRY, FERTILE, RECOVERY, STARVING, WATER_RANGE};
use crate::view::Sown;
use crate::world::{World, invalid};

pub struct Soil {
    /// As of `since`.
    fertility: u32,
    /// When it last lost fertility; it recovers from then while nothing grows on it.
    since: u32,
}

pub struct Planted {
    pub crop: Crop,
    pub ripe_at: u32,
}

impl World {
    /// Fertility now, out of 100; 0 off farmland.
    pub fn fertility(&self, at: Coord) -> u32 {
        let Some(soil) = self.soil.get(&(at.y, at.x)) else {
            return 0;
        };
        if self.crops.contains_key(&(at.y, at.x)) {
            return soil.fertility;
        }
        (soil.fertility + (self.tick - soil.since) / RECOVERY).min(FERTILE)
    }

    /// Water within range of `at`, natural or from a finished well of the team.
    fn watered(&self, at: Coord, team: TeamId) -> bool {
        self.map.water_near(at, WATER_RANGE)
            || self.structures.values().any(|s| {
                s.team == team
                    && s.work_left == 0
                    && s.kind.def().water.is_some_and(|r| s.pos.dist(at) <= r)
            })
    }

    /// Turns grass or dirt in `dir` into farmland.
    pub fn till(&mut self, bot: BotId, dir: Dir) -> Result<(), ActionError> {
        self.can_act()?;
        let at = self.target(bot, dir)?;
        if !matches!(self.map.tile(at), Tile::Grass | Tile::Dirt) {
            return Err(invalid("only grass and dirt can be tilled"));
        }
        if self.structure_on(at).is_some() {
            return Err(invalid("a building stands there"));
        }
        self.map.set(at, Tile::Farmland);
        let soil = Soil {
            fertility: FERTILE,
            since: self.tick,
        };
        self.soil.insert((at.y, at.x), soil);
        self.changes.tiles.insert(at);
        self.turn.acted = true;
        Ok(())
    }

    /// Plants `seed` in watered farmland in `dir`.
    pub fn plant(&mut self, bot: BotId, dir: Dir, seed: Item) -> Result<Sown, ActionError> {
        self.can_act()?;
        let at = self.target(bot, dir)?;
        let crop = Crop::from_seed(seed).ok_or_else(|| invalid("not a seed"))?;
        if self.map.tile(at) != Tile::Farmland || self.crops.contains_key(&(at.y, at.x)) {
            return Err(invalid("not empty farmland"));
        }
        if self.bots[bot].carrying.count(seed) == 0 {
            return Err(invalid("not carrying that seed"));
        }
        if !self.watered(at, self.bots[bot].team()) {
            return Err(invalid("no water within reach"));
        }
        // Rest ends: what it recovered is kept.
        let fertility = self.fertility(at);
        let soil = self.soil.get_mut(&(at.y, at.x)).unwrap();
        (soil.fertility, soil.since) = (fertility, self.tick);
        let ripe_at = self.tick + crop.def().grows;
        self.crops.insert((at.y, at.x), Planted { crop, ripe_at });
        self.bots[bot].carrying.remove(seed, 1);
        self.changes.units.insert(bot);
        self.changes.crops.insert(at);
        self.turn.acted = true;
        Ok(Sown { crop, ripe_at })
    }

    /// Harvests the ripe crop in `dir`, whoever planted it; returns how many items it
    /// gave. The yield scales with the soil's fertility, which the harvest then wears.
    pub fn harvest(&mut self, bot: BotId, dir: Dir) -> Result<u32, ActionError> {
        self.can_act()?;
        let at = self.target(bot, dir)?;
        let planted = self
            .crops
            .get(&(at.y, at.x))
            .ok_or_else(|| invalid("nothing grows there"))?;
        if self.tick < planted.ripe_at {
            return Err(invalid("not ripe"));
        }
        let def = planted.crop.def();
        let fertility = self.fertility(at);
        let gives: Vec<(Item, u32)> = def
            .gives
            .iter()
            .map(|(&item, &n)| (item, n * fertility / FERTILE))
            .collect();
        let total: u32 = gives.iter().map(|&(_, n)| n).sum();
        if total > CARRY - self.bots[bot].carrying.total() {
            return Err(invalid("no room for the harvest"));
        }
        for (item, n) in gives {
            self.bots[bot].carrying.add(item, n);
        }
        let soil = self.soil.get_mut(&(at.y, at.x)).unwrap();
        (soil.fertility, soil.since) = (fertility.saturating_sub(def.tires), self.tick);
        match def.regrows {
            Some(ticks) => self.crops.get_mut(&(at.y, at.x)).unwrap().ripe_at = self.tick + ticks,
            None => drop(self.crops.remove(&(at.y, at.x))),
        }
        self.changes.units.insert(bot);
        self.changes.crops.insert(at);
        self.turn.acted = true;
        Ok(total)
    }

    /// Takes the crop off `at` without a harvest; the soil starts resting.
    pub fn uproot(&mut self, at: Coord) {
        self.crops.remove(&(at.y, at.x));
        if let Some(soil) = self.soil.get_mut(&(at.y, at.x)) {
            soil.since = self.tick;
        }
        self.changes.crops.insert(at);
    }

    /// Every unit due a meal eats the first food it carries, in id order, or loses health.
    /// Returns the units that starved to death.
    pub fn feed(&mut self) -> Vec<BotId> {
        let mut dead = Vec::new();
        for id in self.bots.ids() {
            let unit = &mut self.bots[id];
            if self.tick < unit.hungry_at {
                continue;
            }
            let food = unit
                .carrying
                .0
                .keys()
                .find_map(|&item| Some((item, item.def().food?)));
            match food {
                Some((item, ticks)) => {
                    unit.carrying.remove(item, 1);
                    unit.hungry_at = self.tick + ticks;
                }
                None => {
                    unit.hp = unit.hp.saturating_sub(STARVING);
                    if unit.hp == 0 {
                        dead.push(id);
                    }
                }
            }
            self.changes.units.insert(id);
        }
        dead
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Building;
    use crate::map::Map;
    use crate::rules::{FIRST_MEAL, HP};
    use crate::view::Kind;
    use crate::world::Turn;

    const A: TeamId = TeamId(0);

    /// A 10x10 map, water at (0, 0) and rock at (9, 9); team A's base at (5, 5) and a
    /// worker at (3, 3).
    fn world() -> (World, BotId) {
        let mut rows = [".........."; 10];
        rows[0] = "~.........";
        rows[9] = ".........r";
        let mut w = World::new(Map::from_text(&rows.join(" ")));
        let base = w.place_structure(A, Building::Base, Coord::new(5, 5));
        w.structures.get_mut(&base).unwrap().work_left = 0;
        let worker = w.place_unit(A, Kind::Worker, Coord::new(3, 3));
        (w, worker)
    }

    fn refused<T: std::fmt::Debug>(r: Result<T, ActionError>, why: &str) {
        match r {
            Err(ActionError::Invalid(msg)) => assert!(msg.contains(why), "{msg}"),
            other => panic!("expected a refusal about {why:?}, got {other:?}"),
        }
    }

    fn next_tick(w: &mut World, ticks: u32) {
        w.tick += ticks;
        w.turn = Turn::default();
    }

    #[test]
    fn wheat_is_tilled_planted_near_water_and_harvested_when_ripe() {
        let (mut w, worker) = world();
        w.bots[worker].carrying.add(Item::WheatSeed, 1);
        refused(
            w.plant(worker, Dir::Left, Item::WheatSeed),
            "not empty farmland",
        );
        w.till(worker, Dir::Left).unwrap();
        assert_eq!(w.map.tile(Coord::new(2, 3)), Tile::Farmland);
        next_tick(&mut w, 1);
        refused(w.plant(worker, Dir::Left, Item::Wood), "not a seed");
        let sown = w.plant(worker, Dir::Left, Item::WheatSeed).unwrap();
        assert_eq!(
            sown,
            Sown {
                crop: Crop::Wheat,
                ripe_at: 1 + Crop::Wheat.def().grows
            }
        );
        let ripe_at = sown.ripe_at;
        assert_eq!(w.bots[worker].carrying.total(), 0);
        next_tick(&mut w, 1);
        refused(w.harvest(worker, Dir::Left), "not ripe");
        let wait = ripe_at - w.tick;
        next_tick(&mut w, wait);
        assert_eq!(w.harvest(worker, Dir::Left).unwrap(), 4 + 2);
        assert_eq!(w.bots[worker].carrying.count(Item::Wheat), 4);
        assert!(w.crops.is_empty());
        assert_eq!(
            w.fertility(Coord::new(2, 3)),
            FERTILE - Crop::Wheat.def().tires
        );
        // Rest recovers what the harvest took.
        next_tick(&mut w, RECOVERY * 30);
        assert_eq!(w.fertility(Coord::new(2, 3)), FERTILE);
    }

    #[test]
    fn crops_and_soil_show_in_observations_and_snapshots() {
        let (mut w, worker) = world();
        w.bots[worker].carrying.add(Item::WheatSeed, 1);
        w.till(worker, Dir::Left).unwrap();
        next_tick(&mut w, 0);
        let ripe_at = w.plant(worker, Dir::Left, Item::WheatSeed).unwrap().ripe_at;
        let row = vec![2, 3, Crop::Wheat as u64, u64::from(ripe_at)];
        let seen = w.observe(worker, 9);
        assert_eq!(seen.crops, row);
        assert_eq!(seen.soil, vec![2, 3, u64::from(FERTILE)]);
        assert_eq!(w.snapshot(1, false).crops, row);
        w.changes = Default::default();
        next_tick(&mut w, ripe_at);
        w.harvest(worker, Dir::Left).unwrap();
        let snapshot = w.snapshot(2, false);
        assert!(snapshot.crops.is_empty());
        assert_eq!(snapshot.cleared, vec![2, 3]);
        // Nothing is built on farmland.
        next_tick(&mut w, 0);
        w.bots[worker].carrying.add(Item::Wood, 4);
        refused(
            w.build(worker, Dir::Left, Building::WoodWall),
            "isn't clear",
        );
    }

    #[test]
    fn worn_soil_yields_less() {
        let (mut w, worker) = world();
        w.till(worker, Dir::Left).unwrap();
        w.soil.get_mut(&(3, 2)).unwrap().fertility = 50;
        w.bots[worker].carrying.add(Item::WheatSeed, 1);
        next_tick(&mut w, 0);
        w.plant(worker, Dir::Left, Item::WheatSeed).unwrap();
        next_tick(&mut w, 60);
        assert_eq!(w.harvest(worker, Dir::Left).unwrap(), 2 + 1);
        assert_eq!(w.fertility(Coord::new(2, 3)), 25);
    }

    #[test]
    fn far_from_water_only_a_well_lets_crops_be_planted() {
        let (mut w, worker) = world();
        w.bots[worker].pos = Coord::new(7, 7);
        w.bots[worker].carrying.add(Item::WheatSeed, 1);
        w.till(worker, Dir::Up).unwrap();
        next_tick(&mut w, 0);
        refused(w.plant(worker, Dir::Up, Item::WheatSeed), "no water");
        let well = w.place_structure(A, Building::Well, Coord::new(8, 7));
        refused(w.plant(worker, Dir::Up, Item::WheatSeed), "no water");
        w.structures.get_mut(&well).unwrap().work_left = 0;
        w.plant(worker, Dir::Up, Item::WheatSeed).unwrap();
    }

    #[test]
    fn apple_trees_take_long_then_regrow_fruit() {
        let (mut w, worker) = world();
        w.till(worker, Dir::Left).unwrap();
        w.bots[worker].carrying.add(Item::AppleSeed, 1);
        next_tick(&mut w, 0);
        let ripe_at = w.plant(worker, Dir::Left, Item::AppleSeed).unwrap().ripe_at;
        next_tick(&mut w, ripe_at);
        assert_eq!(w.harvest(worker, Dir::Left).unwrap(), 3);
        let again = w.crops[&(3, 2)].ripe_at;
        assert_eq!(again, w.tick + 80);
        next_tick(&mut w, 1);
        refused(w.harvest(worker, Dir::Left), "not ripe");
    }

    #[test]
    fn harvests_must_fit_and_anyone_may_take_them() {
        let (mut w, worker) = world();
        w.till(worker, Dir::Left).unwrap();
        w.crops.insert(
            (3, 2),
            Planted {
                crop: Crop::Wheat,
                ripe_at: 0,
            },
        );
        let thief = w.place_unit(TeamId(1), Kind::Worker, Coord::new(2, 4));
        w.bots[thief].carrying.add(Item::Stone, CARRY - 5);
        next_tick(&mut w, 0);
        refused(w.harvest(thief, Dir::Up), "no room");
        w.bots[thief].carrying.remove(Item::Stone, 1);
        assert_eq!(w.harvest(thief, Dir::Up).unwrap(), 6);
    }

    #[test]
    fn units_eat_what_they_carry_or_starve() {
        let (mut w, worker) = world();
        w.bots[worker].carrying.add(Item::Wheat, 1);
        w.tick = FIRST_MEAL - 1;
        assert!(w.feed().is_empty());
        assert_eq!(w.bots[worker].carrying.count(Item::Wheat), 1);
        w.tick = FIRST_MEAL;
        w.feed();
        assert_eq!(w.bots[worker].carrying.count(Item::Wheat), 0);
        assert_eq!(w.bots[worker].hungry_at, FIRST_MEAL + 100);
        w.tick = FIRST_MEAL + 100;
        for _ in 1..HP {
            assert!(w.feed().is_empty());
        }
        assert_eq!(w.bots[worker].hp, STARVING);
        assert_eq!(w.feed(), vec![worker]);
    }
}
