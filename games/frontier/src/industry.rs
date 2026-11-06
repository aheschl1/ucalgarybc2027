//! Crafting, drills, and what each team knows. A craft runs as a batch: it takes its
//! inputs when it starts and leaves its outputs in the building's storage when it ends.
//! A team learns a recipe or building once it has studied every item it uses.

use std::collections::{BTreeMap, BTreeSet};

use ucbc_engine::{ActionError, BotId};

use crate::content::{Building, Item, Recipe, content};
use crate::map::Dir;
use crate::rules::CARRY;
use crate::world::{World, invalid};

/// What a team knows how to make and build, and the items it has studied.
pub struct Known {
    pub recipes: BTreeSet<Recipe>,
    pub buildings: BTreeSet<Building>,
    pub studied: BTreeSet<Item>,
}

impl Known {
    pub fn start() -> Self {
        let c = content();
        let recipes = c.recipes.iter().filter(|(_, r)| r.known);
        let buildings = c.buildings.iter().filter(|(_, b)| b.known);
        Self {
            recipes: recipes.map(|(&id, _)| id).collect(),
            buildings: buildings.map(|(&id, _)| id).collect(),
            studied: BTreeSet::new(),
        }
    }

    /// Studies `item`: everything whose items are all studied now is known. Returns what
    /// that adds.
    pub fn study(&mut self, item: Item) -> Learned {
        self.studied.insert(item);
        let c = content();
        let studied = |items: &BTreeMap<Item, u32>| items.keys().all(|i| self.studied.contains(i));
        let recipes: Vec<Recipe> = c
            .recipes
            .iter()
            .filter(|(_, r)| studied(&r.inputs))
            .map(|(&r, _)| r)
            .collect();
        let buildings: Vec<Building> = c
            .buildings
            .iter()
            .filter(|(_, b)| studied(&b.cost))
            .map(|(&b, _)| b)
            .collect();
        Learned {
            recipes: recipes
                .into_iter()
                .filter(|&r| self.recipes.insert(r))
                .collect(),
            buildings: buildings
                .into_iter()
                .filter(|&b| self.buildings.insert(b))
                .collect(),
        }
    }
}

pub struct Job {
    pub recipe: Recipe,
    pub done_at: u32,
}

impl World {
    /// The team's finished building of a kind with `has` in `dir`.
    fn own_building(
        &self,
        bot: BotId,
        dir: Dir,
        has: impl Fn(Building) -> bool,
        what: &str,
    ) -> Result<u64, ActionError> {
        let at = self.target(bot, dir)?;
        let team = self.bots[bot].team();
        self.structure_on(at)
            .filter(|id| {
                let s = &self.structures[id];
                s.team == team && s.work_left == 0 && has(s.kind)
            })
            .ok_or_else(|| ActionError::Invalid(format!("not your team's finished {what}")))
    }

    /// Starts `recipe` at the building in `dir` from what it stores; returns the tick it
    /// ends.
    pub fn craft(&mut self, bot: BotId, dir: Dir, recipe: Recipe) -> Result<u32, ActionError> {
        self.can_act()?;
        let def = recipe.def();
        let id = self.own_building(bot, dir, |b| b == def.at, "building that makes it")?;
        if !self.known(self.bots[bot].team()).recipes.contains(&recipe) {
            return Err(invalid("your team doesn't know that recipe"));
        }
        let s = &self.structures[&id];
        if s.job.is_some() {
            return Err(invalid("busy with another job"));
        }
        if !s.stored.covers(&def.inputs) {
            return Err(invalid("its storage lacks the inputs"));
        }
        let (taken, made) = (
            def.inputs.values().sum::<u32>(),
            def.outputs.values().sum::<u32>(),
        );
        if made > s.room() + taken {
            return Err(invalid("no room in its storage for what it makes"));
        }
        let s = self.structures.get_mut(&id).unwrap();
        for (&item, &n) in &def.inputs {
            s.stored.remove(item, n);
        }
        let done_at = self.tick + def.ticks;
        s.job = Some(Job { recipe, done_at });
        self.jobs.insert((done_at, id));
        self.changes.structures.insert(id);
        self.turn.acted = true;
        Ok(done_at)
    }

    /// Ends the jobs due by now, leaving their outputs in the room kept for them.
    pub fn finish_jobs(&mut self) {
        while let Some(&(done_at, id)) = self.jobs.first().filter(|&&(at, _)| at <= self.tick) {
            self.jobs.remove(&(done_at, id));
            let s = self.structures.get_mut(&id).unwrap();
            let job = s.job.take().unwrap();
            for (&item, &n) in &job.recipe.def().outputs {
                s.stored.add(item, n);
            }
            self.changes.structures.insert(id);
        }
    }

    /// Works the drill in `dir`: takes up to its rate from the tile under it into what
    /// the bot carries, burning one fuel from its storage if it needs fuel. Returns how
    /// many items.
    pub fn operate(&mut self, bot: BotId, dir: Dir) -> Result<u32, ActionError> {
        self.can_act()?;
        let id = self.own_building(bot, dir, |b| b.def().drill.is_some(), "drill")?;
        let s = &self.structures[&id];
        let (pos, drill) = (s.pos, s.kind.def().drill.as_ref().unwrap());
        let tier = self.map.resource(pos).map(|(_, tier)| tier);
        if tier.is_none_or(|t| t > drill.tier) {
            return Err(invalid("nothing under it the drill can reach"));
        }
        if drill.burns.is_some_and(|fuel| s.stored.count(fuel) == 0) {
            return Err(invalid("out of fuel"));
        }
        let room = CARRY - self.bots[bot].carrying.total();
        if room == 0 {
            return Err(invalid("carrying all it can"));
        }
        if let Some(fuel) = drill.burns {
            self.structures.get_mut(&id).unwrap().stored.remove(fuel, 1);
            self.changes.structures.insert(id);
        }
        let before = self.map.tile(pos);
        let mut n = 0;
        while n < drill.rate.min(room) {
            let Some(item) = self.map.gather(pos) else {
                break;
            };
            self.bots[bot].carrying.add(item, 1);
            n += 1;
        }
        if self.map.tile(pos) != before {
            self.changes.tiles.insert(pos);
        }
        self.changes.units.insert(bot);
        self.turn.acted = true;
        Ok(n)
    }

    /// Spends one `item` at a research building in `dir` for the team to study; returns
    /// what that taught it.
    pub fn study(&mut self, bot: BotId, dir: Dir, item: Item) -> Result<Learned, ActionError> {
        self.can_act()?;
        self.own_building(bot, dir, |b| b.def().research, "research building")?;
        if self.bots[bot].carrying.count(item) == 0 {
            return Err(invalid("not carrying that"));
        }
        self.bots[bot].carrying.remove(item, 1);
        self.changes.units.insert(bot);
        self.turn.acted = true;
        Ok(self.known[self.bots[bot].team().0 as usize].study(item))
    }
}

/// What a `study` taught the team.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Learned {
    pub recipes: Vec<Recipe>,
    pub buildings: Vec<Building>,
}

#[cfg(test)]
mod tests {
    use ucbc_engine::TeamId;

    use super::*;
    use crate::map::{Coord, Map};
    use crate::view::Kind;
    use crate::world::Turn;

    const A: TeamId = TeamId(0);

    /// A 6x6 map: copper above team A's worker at (1, 1), iron either side of it.
    fn world() -> (World, BotId) {
        let mut w = World::new(Map::from_text(
            ".u....
             i.i...
             ......
             ......
             ......
             ......",
        ));
        let worker = w.place_unit(A, Kind::Worker, Coord::new(1, 1));
        (w, worker)
    }

    fn refused<T: std::fmt::Debug>(r: Result<T, ActionError>, why: &str) {
        match r {
            Err(ActionError::Invalid(msg)) => assert!(msg.contains(why), "{msg}"),
            other => panic!("expected a refusal about {why:?}, got {other:?}"),
        }
    }

    fn next_step(w: &mut World) {
        w.turn = Turn::default();
    }

    fn finished(w: &mut World, kind: Building, at: Coord) -> u64 {
        let id = w.place_structure(A, kind, at);
        w.structures.get_mut(&id).unwrap().work_left = 0;
        id
    }

    #[test]
    fn a_craft_takes_its_inputs_then_leaves_its_outputs_when_done() {
        let (mut w, worker) = world();
        let bench = finished(&mut w, Building::Workbench, Coord::new(1, 2));
        refused(
            w.craft(worker, Dir::Down, Recipe::StonePick),
            "lacks the inputs",
        );
        w.structures
            .get_mut(&bench)
            .unwrap()
            .stored
            .add(Item::Wood, 2);
        w.structures
            .get_mut(&bench)
            .unwrap()
            .stored
            .add(Item::Stone, 3);
        let done_at = w.craft(worker, Dir::Down, Recipe::StonePick).unwrap();
        assert_eq!(done_at, 10);
        assert!(w.structures[&bench].stored.0.is_empty());
        next_step(&mut w);
        refused(w.craft(worker, Dir::Down, Recipe::StonePick), "busy");
        w.tick = 9;
        w.finish_jobs();
        assert!(w.structures[&bench].stored.0.is_empty());
        w.tick = 10;
        w.finish_jobs();
        assert_eq!(w.structures[&bench].stored.count(Item::StonePick), 1);
        assert!(w.structures[&bench].job.is_none() && w.jobs.is_empty());
        // Wrong building, unknown recipe.
        refused(
            w.craft(worker, Dir::Down, Recipe::Charcoal),
            "not your team's finished",
        );
        let smelter = finished(&mut w, Building::Smelter, Coord::new(2, 1));
        w.structures
            .get_mut(&smelter)
            .unwrap()
            .stored
            .add(Item::CopperOre, 2);
        w.structures
            .get_mut(&smelter)
            .unwrap()
            .stored
            .add(Item::Coal, 1);
        refused(
            w.craft(worker, Dir::Right, Recipe::CopperIngot),
            "doesn't know",
        );
    }

    #[test]
    fn a_team_learns_what_it_has_studied_every_item_of() {
        let (mut w, worker) = world();
        refused(
            w.study(worker, Dir::Down, Item::CopperOre),
            "research building",
        );
        finished(&mut w, Building::ResearchDesk, Coord::new(1, 2));
        refused(w.study(worker, Dir::Down, Item::CopperOre), "not carrying");
        w.bots[worker].carrying.add(Item::CopperOre, 2);
        w.bots[worker].carrying.add(Item::Coal, 1);
        // Copper ore alone: copper ingots also take coal.
        let nothing = Learned {
            recipes: vec![],
            buildings: vec![],
        };
        assert_eq!(
            w.study(worker, Dir::Down, Item::CopperOre).unwrap(),
            nothing
        );
        next_step(&mut w);
        let learned = w.study(worker, Dir::Down, Item::Coal).unwrap();
        assert_eq!(learned.recipes, vec![Recipe::CopperIngot]);
        assert_eq!(w.bots[worker].carrying.count(Item::CopperOre), 1);
        // Again: nothing new. And the other team learned nothing.
        next_step(&mut w);
        assert_eq!(
            w.study(worker, Dir::Down, Item::CopperOre).unwrap(),
            nothing
        );
        assert!(!w.known[1].recipes.contains(&Recipe::CopperIngot));
    }

    #[test]
    fn a_busy_building_keeps_room_for_what_it_makes() {
        let (mut w, worker) = world();
        w.known[0].recipes.insert(Recipe::Gear);
        let bench = finished(&mut w, Building::Workbench, Coord::new(1, 2));
        fn stored(w: &mut World, id: u64) -> &mut crate::world::Items {
            &mut w.structures.get_mut(&id).unwrap().stored
        }
        // One ingot makes two gears: with the bench full, there's no room for them.
        stored(&mut w, bench).add(Item::IronIngot, 1);
        stored(&mut w, bench).add(Item::Coal, 29);
        refused(w.craft(worker, Dir::Down, Recipe::Gear), "no room");
        stored(&mut w, bench).remove(Item::Coal, 1);
        let done_at = w.craft(worker, Dir::Down, Recipe::Gear).unwrap();
        // 28 coal and the two gears to come fill it.
        next_step(&mut w);
        w.bots[worker].carrying.add(Item::Coal, 1);
        refused(w.put(worker, Dir::Down, Item::Coal, 1), "no room");
        let job = w.observe(worker, 3).stores[0].job.unwrap();
        assert_eq!((job.recipe, job.done_at), (Recipe::Gear, done_at));
        w.tick = done_at;
        w.finish_jobs();
        assert_eq!(w.structures[&bench].stored.count(Item::Gear), 2);
        assert_eq!(w.structures[&bench].stored.total(), 30);
    }

    #[test]
    fn tools_and_drills_reach_veins_by_tier() {
        let (mut w, worker) = world();
        // Copper needs a stone pick in hand.
        refused(w.gather(worker, Dir::Up), "better tool");
        w.bots[worker].carrying.add(Item::StonePick, 1);
        assert_eq!(w.gather(worker, Dir::Up).unwrap(), Item::CopperOre);
        // A drill is unknown until learned, and stands only on rock or a vein.
        next_step(&mut w);
        w.bots[worker].carrying.add(Item::Wood, 10);
        w.bots[worker].carrying.add(Item::CopperIngot, 4);
        refused(
            w.build(worker, Dir::Up, Building::MechDrill),
            "doesn't know",
        );
        w.known[0].buildings.insert(Building::MechDrill);
        refused(
            w.build(worker, Dir::Down, Building::MechDrill),
            "rock or a vein",
        );
        w.build(worker, Dir::Up, Building::MechDrill).unwrap();
        next_step(&mut w);
        refused(w.operate(worker, Dir::Up), "not your team's finished drill");
        let drill = w.structure_on(Coord::new(1, 0)).unwrap();
        w.structures.get_mut(&drill).unwrap().work_left = 0;
        w.bots[worker].carrying = Default::default();
        assert_eq!(w.operate(worker, Dir::Up).unwrap(), 3);
        assert_eq!(w.bots[worker].carrying.count(Item::CopperOre), 3);
        // Iron is beyond a mechanical drill; a steam drill reaches it, burning coal.
        next_step(&mut w);
        finished(&mut w, Building::MechDrill, Coord::new(0, 1));
        refused(w.operate(worker, Dir::Left), "drill can reach");
        let steam = finished(&mut w, Building::SteamDrill, Coord::new(2, 1));
        refused(w.operate(worker, Dir::Right), "out of fuel");
        w.structures
            .get_mut(&steam)
            .unwrap()
            .stored
            .add(Item::Coal, 1);
        assert_eq!(w.operate(worker, Dir::Right).unwrap(), 6);
        assert_eq!(w.structures[&steam].stored.total(), 0);
        // The copper runs out under the first drill: it turns to dirt, and the drill stops.
        w.bots[worker].carrying = Default::default();
        for _ in 0..12 {
            next_step(&mut w);
            w.operate(worker, Dir::Up).unwrap();
            w.bots[worker].carrying = Default::default();
        }
        assert_eq!(w.map.tile(Coord::new(1, 0)), crate::map::Tile::Dirt);
        assert!(w.changes.tiles.contains(&Coord::new(1, 0)));
        next_step(&mut w);
        refused(w.operate(worker, Dir::Up), "drill can reach");
    }

    #[test]
    fn a_drill_never_shares_a_tile() {
        let mut w = World::new(Map::from_text("f.. ... ..."));
        let worker = w.place_unit(A, Kind::Worker, Coord::new(1, 0));
        w.place_unit(A, Kind::Worker, Coord::new(0, 0));
        w.known[0].buildings.insert(Building::MechDrill);
        w.bots[worker].carrying.add(Item::Wood, 10);
        w.bots[worker].carrying.add(Item::CopperIngot, 4);
        // Forest is walkable: someone may stand on it.
        refused(
            w.build(worker, Dir::Left, Building::MechDrill),
            "rock or a vein",
        );
    }
}
