//! Fighting. A hit does the attacker's best weapon less the target's best armour, at
//! least 1, halved for a unit manning a tower. What dies or falls leaves what it held.

use ucbc_engine::{ActionError, BotId, TeamId};

use crate::content::Building;
use crate::map::{Coord, Dir};
use crate::rules::FIST;
use crate::view::Hit;
use crate::world::{World, invalid};

impl World {
    /// The team's finished towers next to `at`, in the four directions.
    fn towers_by(&self, at: Coord, team: TeamId) -> impl Iterator<Item = (Coord, usize)> + '_ {
        Dir::ALL.into_iter().filter_map(move |dir| {
            let s = &self.structures[&self.structure_on(self.map.step(at, dir)?)?];
            let range = s.kind.def().tower?;
            (s.team == team && s.work_left == 0).then_some((s.pos, range))
        })
    }

    /// Hits what is on `at`: a unit of the other team, a building of the other team, or
    /// failing those, a crop, which is destroyed. `at` must be next to the bot, or in
    /// range of a tower of its team next to it. Returns the hit and the unit it killed.
    pub fn attack(&mut self, bot: BotId, at: Coord) -> Result<(Hit, Option<BotId>), ActionError> {
        self.can_act()?;
        let (me, team) = (self.bots[bot].pos, self.bots[bot].team());
        if at.x >= self.map.size() || at.y >= self.map.size() {
            return Err(invalid("off the map"));
        }
        let beside = me.x.abs_diff(at.x) + me.y.abs_diff(at.y) == 1;
        if !beside
            && !self
                .towers_by(me, team)
                .any(|(pos, range)| pos.dist(at) <= range)
        {
            return Err(invalid("out of reach"));
        }
        let weapon = self.bots[bot].carrying.best(|d| d.weapon).unwrap_or(FIST);
        let hit = |armour: u32| weapon.saturating_sub(armour).max(1);
        if let Some(target) = self.unit_on(at) {
            if self.bots[target].team() == team {
                return Err(invalid("that's your team's"));
            }
            let mut damage = hit(self.bots[target].carrying.best(|d| d.armour).unwrap_or(0));
            if self.towers_by(at, self.bots[target].team()).count() > 0 {
                damage = damage.div_ceil(2);
            }
            let unit = &mut self.bots[target];
            unit.hp = unit.hp.saturating_sub(damage);
            let left = unit.hp;
            self.changes.units.insert(target);
            self.turn.acted = true;
            let killed = (left == 0).then(|| {
                self.remove_unit(target);
                self.bots.remove(target);
                target
            });
            return Ok((Hit { damage, left }, killed));
        }
        if let Some(id) = self.visible_structure(at, team) {
            if self.structures[&id].team == team {
                return Err(invalid("that's your team's"));
            }
            let damage = hit(0);
            let s = self.structures.get_mut(&id).unwrap();
            s.hp = s.hp.saturating_sub(damage);
            let left = s.hp;
            self.changes.structures.insert(id);
            if left == 0 {
                self.destroy(id);
            }
            self.turn.acted = true;
            return Ok((Hit { damage, left }, None));
        }
        if self.crops.contains_key(&(at.y, at.x)) {
            self.uproot(at);
            self.turn.acted = true;
            return Ok((Hit { damage: 0, left: 0 }, None));
        }
        Err(invalid("nothing to attack there"))
    }

    /// Takes a building down, leaving what it stored, or for a site what was paid and
    /// delivered for it, on its tile if things can lie there.
    pub(crate) fn destroy(&mut self, id: u64) {
        let mut s = self.structures.remove(&id).unwrap();
        if s.work_left > 0 {
            for (&item, &n) in &s.kind.def().cost {
                s.stored.add(item, n - s.owed.count(item));
            }
        }
        if let Some(job) = &s.job {
            self.jobs.remove(&(job.done_at, id));
        }
        if s.kind == Building::Base {
            self.bases[s.team.0 as usize] = None;
        }
        let i = self.idx(s.pos);
        self.structure_at[i] = 0;
        if self.map.tile(s.pos).walkable() {
            self.drop_items(s.pos, s.stored);
        }
        self.changes.structures.remove(&id);
        self.changes.demolished.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Crop, Item};
    use crate::farm::Planted;
    use crate::map::Map;
    use crate::rules::HP;
    use crate::view::Kind;
    use crate::world::{Items, Turn};

    const A: TeamId = TeamId(0);
    const B: TeamId = TeamId(1);

    /// A grassy 14x14 map: team A's worker at (5, 5), team B's at (6, 5).
    fn world() -> (World, BotId, BotId) {
        let mut w = World::new(Map::from_text(&vec![".".repeat(14); 14].join(" ")));
        let a = w.place_unit(A, Kind::Worker, Coord::new(5, 5));
        let b = w.place_unit(B, Kind::Worker, Coord::new(6, 5));
        (w, a, b)
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

    fn finished(w: &mut World, team: TeamId, kind: Building, at: Coord) -> u64 {
        let id = w.place_structure(team, kind, at);
        w.structures.get_mut(&id).unwrap().work_left = 0;
        id
    }

    #[test]
    fn weapons_hit_harder_and_armour_takes_some_off() {
        let (mut w, a, b) = world();
        let (hit, killed) = w.attack(a, Coord::new(6, 5)).unwrap();
        assert_eq!((hit.damage, hit.left, killed), (FIST, HP - FIST, None));
        refused(w.attack(a, Coord::new(6, 5)), "already acted");
        next_step(&mut w);
        w.bots[a].carrying.add(Item::IronSword, 1);
        w.bots[b].carrying.add(Item::CopperArmour, 1);
        assert_eq!(w.attack(a, Coord::new(6, 5)).unwrap().0.damage, 18 - 4);
        next_step(&mut w);
        refused(w.attack(a, Coord::new(7, 5)), "out of reach");
        refused(w.attack(a, Coord::new(4, 5)), "nothing to attack");
        let mate = w.place_unit(A, Kind::Worker, Coord::new(5, 6));
        refused(w.attack(a, w.bots[mate].pos), "your team's");
    }

    #[test]
    fn a_killed_unit_drops_what_it_carried() {
        let (mut w, a, b) = world();
        w.bots[b].hp = 2;
        w.bots[b].carrying.add(Item::Wheat, 4);
        let (hit, killed) = w.attack(a, Coord::new(6, 5)).unwrap();
        assert_eq!((hit.left, killed), (0, Some(b)));
        assert!(w.bots.ids().iter().all(|&id| id != b));
        assert_eq!(w.unit_on(Coord::new(6, 5)), None);
        assert_eq!(w.piles[&(5, 6)].count(Item::Wheat), 4);
        assert_eq!(w.changes.removed, vec![b.0]);
    }

    #[test]
    fn a_destroyed_building_spills_its_storage_and_a_base_stops_counting() {
        let (mut w, a, b) = world();
        let base = finished(&mut w, B, Building::Base, Coord::new(5, 4));
        w.structures.get_mut(&base).unwrap().hp = 3;
        w.structures
            .get_mut(&base)
            .unwrap()
            .stored
            .add(Item::Wood, 7);
        assert_eq!(w.attack(a, Coord::new(5, 4)).unwrap().0.left, 0);
        assert!(w.base(B).is_none());
        assert_eq!(w.structure_on(Coord::new(5, 4)), None);
        assert_eq!(w.piles[&(4, 5)].count(Item::Wood), 7);
        assert_eq!(w.changes.demolished, vec![base]);
        // Only the commander rebuilds it.
        next_step(&mut w);
        w.bots[b].carrying.add(Item::Wood, 12);
        w.bots[b].carrying.add(Item::Stone, 8);
        refused(w.build(b, Dir::Down, Building::Base), "only the commander");
        let commander = w.place_unit(B, Kind::Commander, Coord::new(8, 8));
        w.bots[commander].carrying = w.bots[b].carrying.clone();
        w.build(commander, Dir::Up, Building::Base).unwrap();
        assert!(w.base(B).is_some());
        next_step(&mut w);
        refused(w.build(commander, Dir::Left, Building::Base), "has a base");
    }

    #[test]
    fn a_destroyed_site_leaves_what_was_paid_and_delivered() {
        let (mut w, a, _) = world();
        let site = w.place_structure(B, Building::Chest, Coord::new(5, 6));
        let s = w.structures.get_mut(&site).unwrap();
        (s.hp, s.owed) = (1, Items([(Item::Wood, 4)].into_iter().collect()));
        w.attack(a, Coord::new(5, 6)).unwrap();
        assert_eq!(w.piles[&(6, 5)].count(Item::Wood), 10 - 4);
    }

    #[test]
    fn a_tower_reaches_far_and_shields_who_mans_it() {
        let (mut w, a, b) = world();
        finished(&mut w, A, Building::Tower, Coord::new(5, 6));
        let far = w.place_unit(B, Kind::Worker, Coord::new(10, 10));
        // In range of the tower (5 from (5, 6)), though far from the attacker.
        assert_eq!(w.attack(a, Coord::new(10, 10)).unwrap().0.damage, FIST);
        next_step(&mut w);
        refused(w.attack(a, Coord::new(12, 12)), "out of reach");
        // B's worker hits A's, who mans the tower: half, rounded up.
        w.bots[b].carrying.add(Item::IronSword, 1);
        assert_eq!(w.attack(b, Coord::new(5, 5)).unwrap().0.damage, 9);
        assert_eq!(w.bots[far].hp, HP - FIST);
    }

    #[test]
    fn gates_let_their_team_through_and_crops_burn() {
        let (mut w, a, b) = world();
        finished(&mut w, A, Building::Gate, Coord::new(5, 4));
        w.move_unit(a, Dir::Up).unwrap();
        assert_eq!(w.bots[a].pos, Coord::new(5, 4));
        next_step(&mut w);
        w.move_unit(a, Dir::Up).unwrap();
        // Off the gate again; things dropped on it can be picked up.
        w.drop_items(
            Coord::new(5, 4),
            crate::world::Items(std::iter::once((Item::Wood, 1)).collect()),
        );
        next_step(&mut w);
        assert_eq!(w.take(a, Dir::Down, Item::Wood, 1).unwrap(), 1);
        // The other team can't walk onto the empty gate.
        next_step(&mut w);
        w.move_unit(b, Dir::Up).unwrap();
        next_step(&mut w);
        refused(w.move_unit(b, Dir::Left), "blocked");
        // A crop: anyone may destroy it.
        w.crops.insert(
            (3, 6),
            Planted {
                crop: Crop::Wheat,
                ripe_at: 0,
            },
        );
        assert_eq!(w.attack(b, Coord::new(6, 3)).unwrap().0.left, 0);
        assert!(w.crops.is_empty());
    }

    #[test]
    fn stealing_from_storage_and_repairing() {
        let (mut w, a, b) = world();
        let chest = finished(&mut w, A, Building::Chest, Coord::new(6, 4));
        w.structures
            .get_mut(&chest)
            .unwrap()
            .stored
            .add(Item::IronIngot, 3);
        assert_eq!(w.take(b, Dir::Up, Item::IronIngot, 5).unwrap(), 3);
        next_step(&mut w);
        refused(
            w.put(b, Dir::Up, Item::IronIngot, 1),
            "not your team's storage",
        );
        // A repairs its damaged chest, up to whole.
        w.structures.get_mut(&chest).unwrap().hp = 47;
        w.move_unit(a, Dir::Up).unwrap();
        w.build(a, Dir::Right, Building::Chest).unwrap();
        assert_eq!(w.structures[&chest].hp, 50);
        next_step(&mut w);
        refused(w.build(a, Dir::Right, Building::Chest), "another building");
    }

    #[test]
    fn a_trap_is_hidden_from_the_other_team_and_hurts_who_steps_on_it() {
        let (mut w, a, b) = world();
        let trap = finished(&mut w, A, Building::SpikeTrap, Coord::new(6, 4));
        let rows = |w: &World, bot| w.observe(bot, 3).buildings;
        assert!(rows(&w, b).is_empty());
        assert_eq!(rows(&w, a)[0], trap);
        // Hidden, so it can't be hit, and things put on it lie on the ground.
        refused(w.attack(b, Coord::new(6, 4)), "nothing to attack");
        w.bots[b].carrying.add(Item::Wood, 1);
        assert_eq!(w.put(b, Dir::Up, Item::Wood, 1).unwrap(), 1);
        next_step(&mut w);
        assert_eq!(w.take(b, Dir::Up, Item::Wood, 1).unwrap(), 1);
        // Stepping on it: 30 damage, and it's spent.
        let (to, killed) = w.move_unit(b, Dir::Up).unwrap();
        assert_eq!((to, killed), (Coord::new(6, 4), None));
        assert_eq!(w.bots[b].hp, HP - 30);
        assert_eq!(w.structure_on(Coord::new(6, 4)), None);
        assert_eq!(w.changes.demolished, vec![trap]);
    }

    #[test]
    fn a_trap_spares_its_team_and_can_kill() {
        let (mut w, a, b) = world();
        finished(&mut w, A, Building::SpikeTrap, Coord::new(4, 5));
        w.move_unit(a, Dir::Left).unwrap();
        assert_eq!(w.bots[a].hp, HP);
        assert!(w.structure_on(Coord::new(4, 5)).is_some());
        // An unfinished one is a site like any other: seen, and in the way.
        next_step(&mut w);
        w.place_structure(A, Building::SpikeTrap, Coord::new(7, 5));
        assert_eq!(w.observe(b, 3).buildings.len(), 7);
        refused(w.move_unit(b, Dir::Right), "blocked");
        // A finished one kills a unit with little health left, which drops what it carried.
        finished(&mut w, A, Building::SpikeTrap, Coord::new(6, 6));
        w.bots[b].hp = 30;
        w.bots[b].carrying.add(Item::Wheat, 2);
        assert_eq!(
            w.move_unit(b, Dir::Down).unwrap(),
            (Coord::new(6, 6), Some(b))
        );
        assert_eq!(w.piles[&(6, 6)].count(Item::Wheat), 2);
    }

    /// Whether a trap is there or not, the other team's actions get the same replies.
    #[test]
    fn actions_on_a_hidden_trap_reveal_nothing() {
        let replies = |trapped: bool| {
            let mut w = World::new(Map::from_text("...... ...f.. ...... ...... ...... ......"));
            let b = w.place_unit(B, Kind::Worker, Coord::new(3, 2));
            if trapped {
                for at in [Coord::new(3, 1), Coord::new(2, 2), Coord::new(4, 2)] {
                    finished(&mut w, A, Building::SpikeTrap, at);
                }
            }
            w.bots[b].carrying.add(Item::Wood, 10);
            let mut out = vec![format!("{:?}", w.gather(b, Dir::Up))];
            next_step(&mut w);
            out.push(format!("{:?}", w.till(b, Dir::Left)));
            next_step(&mut w);
            out.push(format!("{:?}", w.build(b, Dir::Right, Building::WoodWall)));
            out
        };
        assert_eq!(replies(true), replies(false));
    }
}
