//! The set's state and the rules of each action. Each action checks everything before
//! it changes anything, so a refused action leaves the set as it was.

use std::collections::{BTreeMap, BTreeSet};

use ucbc_engine::{ActionError, BotId, BotManager, TeamId};

use crate::content::{Building, Item};
use crate::map::{Coord, Dir, Map};
use crate::rules::{CARRY, MESSAGE_BYTES, SAY_RANGE, SPAWN_COST};
use crate::view::Kind;

fn invalid(message: &str) -> ActionError {
    ActionError::Invalid(message.into())
}

/// Items of any kinds, counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Items(pub BTreeMap<Item, u32>);

impl Items {
    pub fn total(&self) -> u32 {
        self.0.values().sum()
    }

    pub fn count(&self, item: Item) -> u32 {
        self.0.get(&item).copied().unwrap_or(0)
    }

    pub fn add(&mut self, item: Item, n: u32) {
        if n > 0 {
            *self.0.entry(item).or_default() += n;
        }
    }

    /// Removes `n`, which the caller has checked are there.
    pub fn remove(&mut self, item: Item, n: u32) {
        let left = self.count(item) - n;
        if left == 0 {
            self.0.remove(&item);
        } else {
            self.0.insert(item, left);
        }
    }

    pub fn covers(&self, cost: &BTreeMap<Item, u32>) -> bool {
        cost.iter().all(|(&item, &n)| self.count(item) >= n)
    }
}

pub struct Unit {
    pub kind: Kind,
    pub pos: Coord,
    pub carrying: Items,
}

/// A placed building, finished once `work_left` is 0.
pub struct Structure {
    pub kind: Building,
    pub team: TeamId,
    pub pos: Coord,
    pub hp: u32,
    pub work_left: u32,
    pub stored: Items,
}

impl Structure {
    pub fn capacity(&self) -> Option<u32> {
        (self.work_left == 0)
            .then_some(self.kind.def().storage)
            .flatten()
    }

    fn radio(&self) -> Option<usize> {
        (self.work_left == 0)
            .then_some(self.kind.def().radio)
            .flatten()
    }
}

/// A `say`, heard by teammates the tick after.
pub struct Said {
    pub team: TeamId,
    pub from: BotId,
    pub at: Coord,
    pub data: Vec<u8>,
}

/// Where `put` and `take` move items to and from.
#[derive(Clone, Copy)]
enum Holder {
    Unit(BotId),
    Store(u64),
    Ground(Coord),
}

/// What the stepping bot has done this step: one move, one other action, one `say`.
#[derive(Default)]
pub struct Turn {
    pub moved: bool,
    pub acted: bool,
    pub said: bool,
}

/// Since the last snapshot.
#[derive(Default)]
pub struct Changes {
    pub units: BTreeSet<BotId>,
    pub removed: Vec<u64>,
    pub tiles: BTreeSet<Coord>,
    pub structures: BTreeSet<u64>,
    pub piles: BTreeSet<Coord>,
    pub signs: BTreeSet<Coord>,
}

pub struct World {
    pub map: Map,
    pub bots: BotManager<Unit>,
    /// Bot id + 1 on each tile, 0 if none.
    pub unit_at: Vec<u64>,
    pub structures: BTreeMap<u64, Structure>,
    /// Structure id + 1 on each tile, 0 if none.
    pub structure_at: Vec<u64>,
    next_structure: u64,
    /// Keyed (y, x), so a row of the map is a range.
    pub piles: BTreeMap<(usize, usize), Items>,
    pub signs: BTreeMap<(usize, usize), Vec<u8>>,
    /// Base structure ids, indexed by team id.
    pub bases: [u64; 2],
    pub said: Vec<Said>,
    pub heard: Vec<Said>,
    pub turn: Turn,
    pub changes: Changes,
}

impl World {
    pub fn new(map: Map) -> Self {
        let tiles = map.size() * map.size();
        Self {
            map,
            bots: BotManager::new(),
            unit_at: vec![0; tiles],
            structures: BTreeMap::new(),
            structure_at: vec![0; tiles],
            next_structure: 0,
            piles: BTreeMap::new(),
            signs: BTreeMap::new(),
            bases: [0; 2],
            said: Vec::new(),
            heard: Vec::new(),
            turn: Turn::default(),
            changes: Changes::default(),
        }
    }

    pub fn idx(&self, at: Coord) -> usize {
        at.y * self.map.size() + at.x
    }

    pub fn unit_on(&self, at: Coord) -> Option<BotId> {
        self.unit_at[self.idx(at)].checked_sub(1).map(BotId)
    }

    pub fn structure_on(&self, at: Coord) -> Option<u64> {
        self.structure_at[self.idx(at)].checked_sub(1)
    }

    /// Walkable, with no unit or structure on it.
    fn free(&self, at: Coord) -> bool {
        self.map.tile(at).walkable()
            && self.unit_on(at).is_none()
            && self.structure_on(at).is_none()
    }

    pub fn base(&self, team: TeamId) -> &Structure {
        &self.structures[&self.bases[team.0 as usize]]
    }

    pub fn place_unit(&mut self, team: TeamId, kind: Kind, at: Coord) -> BotId {
        let carrying = Items::default();
        let id = self.bots.spawn(
            team,
            Unit {
                kind,
                pos: at,
                carrying,
            },
        );
        let i = self.idx(at);
        self.unit_at[i] = id.0 + 1;
        self.changes.units.insert(id);
        id
    }

    pub fn place_structure(&mut self, team: TeamId, kind: Building, pos: Coord) -> u64 {
        let id = self.next_structure;
        self.next_structure += 1;
        let def = kind.def();
        let stored = Items::default();
        let s = Structure {
            kind,
            team,
            pos,
            hp: def.hp,
            work_left: def.work,
            stored,
        };
        self.structures.insert(id, s);
        let i = self.idx(pos);
        self.structure_at[i] = id + 1;
        self.changes.structures.insert(id);
        id
    }

    /// Takes the bot out of the set, leaving what it carried on its tile.
    pub fn remove_unit(&mut self, bot: BotId) {
        let pos = self.bots[bot].pos;
        let i = self.idx(pos);
        self.unit_at[i] = 0;
        let carrying = std::mem::take(&mut self.bots[bot].carrying);
        if carrying.total() > 0 {
            let pile = self.piles.entry((pos.y, pos.x)).or_default();
            for (item, n) in carrying.0 {
                pile.add(item, n);
            }
            self.changes.piles.insert(pos);
        }
        self.changes.units.remove(&bot);
        self.changes.removed.push(bot.0);
    }

    /// Refused if the bot already used its action this step.
    fn can_act(&self) -> Result<(), ActionError> {
        if self.turn.acted {
            return Err(invalid("already acted this step"));
        }
        Ok(())
    }

    fn target(&self, bot: BotId, dir: Dir) -> Result<Coord, ActionError> {
        self.map
            .step(self.bots[bot].pos, dir)
            .ok_or_else(|| invalid("off the map"))
    }

    pub fn move_unit(&mut self, bot: BotId, dir: Dir) -> Result<Coord, ActionError> {
        if self.turn.moved {
            return Err(invalid("already moved this step"));
        }
        let from = self.bots[bot].pos;
        let to = self.target(bot, dir)?;
        if !self.free(to) {
            return Err(invalid("blocked"));
        }
        let (a, b) = (self.idx(from), self.idx(to));
        self.unit_at.swap(a, b);
        self.bots[bot].pos = to;
        self.turn.moved = true;
        self.changes.units.insert(bot);
        Ok(to)
    }

    pub fn gather(&mut self, bot: BotId, dir: Dir) -> Result<Item, ActionError> {
        self.can_act()?;
        let at = self.target(bot, dir)?;
        if self.bots[bot].carrying.total() >= CARRY {
            return Err(invalid("carrying all it can"));
        }
        if self.structure_on(at).is_some() || self.map.resource(at).is_none() {
            return Err(invalid("nothing to gather there"));
        }
        let before = self.map.tile(at);
        let item = self.map.gather(at).expect("checked above");
        if self.map.tile(at) != before {
            self.changes.tiles.insert(at);
        }
        self.bots[bot].carrying.add(item, 1);
        self.changes.units.insert(bot);
        self.turn.acted = true;
        Ok(item)
    }

    /// What `put` and `take` reach on the tile in `dir`: a teammate, a finished storage
    /// building of the team, or the ground.
    fn holder(&self, bot: BotId, dir: Dir) -> Result<Holder, ActionError> {
        let at = self.target(bot, dir)?;
        let team = self.bots[bot].team();
        match (self.unit_on(at), self.structure_on(at)) {
            (Some(other), _) if self.bots[other].team() == team => Ok(Holder::Unit(other)),
            (Some(_), _) => Err(invalid("that unit is on another team")),
            (None, Some(id)) => {
                let s = &self.structures[&id];
                match s.capacity() {
                    Some(_) if s.team == team => Ok(Holder::Store(id)),
                    _ => Err(invalid("not a storage building of your team")),
                }
            }
            (None, None) if self.map.tile(at).walkable() => Ok(Holder::Ground(at)),
            (None, None) => Err(invalid("nothing can lie there")),
        }
    }

    fn held(&mut self, holder: Holder) -> &mut Items {
        match holder {
            Holder::Unit(id) => &mut self.bots[id].carrying,
            Holder::Store(id) => &mut self.structures.get_mut(&id).unwrap().stored,
            Holder::Ground(at) => self.piles.entry((at.y, at.x)).or_default(),
        }
    }

    fn touched(&mut self, holder: Holder) {
        match holder {
            Holder::Unit(id) => drop(self.changes.units.insert(id)),
            Holder::Store(_) => {}
            Holder::Ground(at) => {
                if self
                    .piles
                    .get(&(at.y, at.x))
                    .is_some_and(|p| p.total() == 0)
                {
                    self.piles.remove(&(at.y, at.x));
                }
                self.changes.piles.insert(at);
            }
        }
    }

    /// Room left in a holder; the ground has no limit.
    fn room(&self, holder: Holder) -> u32 {
        match holder {
            Holder::Unit(id) => CARRY - self.bots[id].carrying.total(),
            Holder::Store(id) => {
                let s = &self.structures[&id];
                s.capacity().unwrap() - s.stored.total()
            }
            Holder::Ground(_) => u32::MAX,
        }
    }

    /// Moves up to `n` of `item` from the bot to the holder in `dir`; returns how many.
    pub fn put(&mut self, bot: BotId, dir: Dir, item: Item, n: u32) -> Result<u32, ActionError> {
        self.can_act()?;
        let to = self.holder(bot, dir)?;
        let n = n
            .min(self.bots[bot].carrying.count(item))
            .min(self.room(to));
        if n == 0 {
            return Err(invalid("nothing to put, or no room"));
        }
        self.bots[bot].carrying.remove(item, n);
        self.held(to).add(item, n);
        self.touched(to);
        self.changes.units.insert(bot);
        self.turn.acted = true;
        Ok(n)
    }

    /// Moves up to `n` of `item` from the holder in `dir` to the bot; returns how many.
    pub fn take(&mut self, bot: BotId, dir: Dir, item: Item, n: u32) -> Result<u32, ActionError> {
        self.can_act()?;
        let from = self.holder(bot, dir)?;
        let there = match from {
            Holder::Ground(at) => self.piles.get(&(at.y, at.x)).map_or(0, |p| p.count(item)),
            _ => self.held(from).count(item),
        };
        let n = n.min(there).min(CARRY - self.bots[bot].carrying.total());
        if n == 0 {
            return Err(invalid("nothing to take, or no room"));
        }
        self.held(from).remove(item, n);
        self.touched(from);
        self.bots[bot].carrying.add(item, n);
        self.changes.units.insert(bot);
        self.turn.acted = true;
        Ok(n)
    }

    /// Places `kind` on the free tile in `dir`, paying its cost from what the bot carries,
    /// or works on the team's unfinished `kind` there. Returns the work left.
    pub fn build(&mut self, bot: BotId, dir: Dir, kind: Building) -> Result<u32, ActionError> {
        self.can_act()?;
        let at = self.target(bot, dir)?;
        let team = self.bots[bot].team();
        if kind == Building::Base {
            return Err(invalid("a base can't be built"));
        }
        if let Some(id) = self.structure_on(at) {
            let s = &self.structures[&id];
            if s.team != team || s.kind != kind || s.work_left == 0 {
                return Err(invalid("another building stands there"));
            }
            let s = self.structures.get_mut(&id).unwrap();
            s.work_left -= 1;
            let left = s.work_left;
            self.changes.structures.insert(id);
            self.turn.acted = true;
            return Ok(left);
        }
        let cost = &kind.def().cost;
        if !self.free(at) || self.piles.contains_key(&(at.y, at.x)) {
            return Err(invalid("the tile isn't clear"));
        }
        if !self.bots[bot].carrying.covers(cost) {
            return Err(invalid("not carrying what it costs"));
        }
        for (&item, &n) in cost {
            self.bots[bot].carrying.remove(item, n);
        }
        self.changes.units.insert(bot);
        let id = self.place_structure(team, kind, at);
        self.turn.acted = true;
        Ok(self.structures[&id].work_left)
    }

    /// A worker on a free tile next to the base, paid from its storage. The commander
    /// must stand next to the base.
    pub fn spawn(&mut self, bot: BotId, cap: u32) -> Result<u64, ActionError> {
        self.can_act()?;
        let team = self.bots[bot].team();
        if self.bots[bot].kind != Kind::Commander {
            return Err(invalid("only a commander can spawn"));
        }
        let base = self.base(team);
        let (pos, base_id) = (base.pos, self.bases[team.0 as usize]);
        if self.bots[bot].pos.dist(pos) != 1 {
            return Err(invalid("the commander must stand next to the base"));
        }
        let (item, n) = SPAWN_COST;
        if base.stored.count(item) < n {
            return Err(invalid("the base can't pay for it"));
        }
        let ids = self.bots.ids();
        if ids
            .iter()
            .filter(|&&id| self.bots[id].team() == team)
            .count() as u32
            >= cap
        {
            return Err(invalid("the team is at its unit cap"));
        }
        let last = self.map.size() - 1;
        let at = (pos.y.saturating_sub(1)..=(pos.y + 1).min(last))
            .flat_map(|y| {
                (pos.x.saturating_sub(1)..=(pos.x + 1).min(last)).map(move |x| Coord::new(x, y))
            })
            .find(|&at| self.free(at))
            .ok_or_else(|| invalid("no free tile next to the base"))?;
        self.structures
            .get_mut(&base_id)
            .unwrap()
            .stored
            .remove(item, n);
        self.turn.acted = true;
        Ok(self.place_unit(team, Kind::Worker, at).0)
    }

    pub fn say(&mut self, bot: BotId, data: Vec<u8>) -> Result<(), ActionError> {
        if self.turn.said {
            return Err(invalid("already said something this step"));
        }
        if data.len() > MESSAGE_BYTES {
            return Err(invalid("longer than a message can be"));
        }
        let (team, at) = (self.bots[bot].team(), self.bots[bot].pos);
        self.said.push(Said {
            team,
            from: bot,
            at,
            data,
        });
        self.turn.said = true;
        Ok(())
    }

    /// Leaves a sign on the bot's tile, replacing any there; empty removes it.
    pub fn mark(&mut self, bot: BotId, data: Vec<u8>) -> Result<(), ActionError> {
        self.can_act()?;
        if data.len() > MESSAGE_BYTES {
            return Err(invalid("longer than a sign can be"));
        }
        let at = self.bots[bot].pos;
        if data.is_empty() {
            self.signs.remove(&(at.y, at.x));
        } else {
            self.signs.insert((at.y, at.x), data);
        }
        self.changes.signs.insert(at);
        self.turn.acted = true;
        Ok(())
    }

    /// Whether `said` reaches `listener`: a teammate within the speaker's own range, or
    /// with both on the team's radio network.
    pub fn hears(&self, listener: BotId, said: &Said) -> bool {
        let (team, at) = (self.bots[listener].team(), self.bots[listener].pos);
        if team != said.team || listener == said.from {
            return false;
        }
        let on_network = |p: Coord| {
            self.structures
                .values()
                .any(|s| s.team == team && s.radio().is_some_and(|r| s.pos.dist(p) <= r))
        };
        at.dist(said.at) <= SAY_RANGE || (on_network(at) && on_network(said.at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::START;

    const A: TeamId = TeamId(0);
    const B: TeamId = TeamId(1);

    /// A 6x6 map: the base at (1, 1), forest at (3, 0), rock at (0, 4), water at (5, 5).
    /// Team A's commander stands at (1, 2) and a worker at (2, 0); team B's worker at (4, 1).
    fn world() -> (World, BotId, BotId, BotId) {
        let mut w = World::new(Map::from_text(
            "...f..
             ......
             ......
             ......
             r.....
             .....~",
        ));
        let base = w.place_structure(A, Building::Base, Coord::new(1, 1));
        w.bases = [base, base];
        w.structures
            .get_mut(&base)
            .unwrap()
            .stored
            .add(START.0, START.1);
        let commander = w.place_unit(A, Kind::Commander, Coord::new(1, 2));
        let worker = w.place_unit(A, Kind::Worker, Coord::new(2, 0));
        let enemy = w.place_unit(B, Kind::Worker, Coord::new(4, 1));
        w.changes = Changes::default();
        (w, commander, worker, enemy)
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

    #[test]
    fn gathering_takes_one_item_an_action_and_runs_a_tile_out() {
        let (mut w, _, worker, _) = world();
        refused(w.gather(worker, Dir::Left), "nothing to gather");
        assert!(!w.turn.acted, "a refusal uses nothing");
        assert_eq!(w.gather(worker, Dir::Right).unwrap(), Item::Wood);
        refused(w.gather(worker, Dir::Right), "already acted");
        for _ in 1..20 {
            next_step(&mut w);
            w.gather(worker, Dir::Right).unwrap();
        }
        assert_eq!(w.map.tile(Coord::new(3, 0)), crate::map::Tile::Dirt);
        assert!(w.changes.tiles.contains(&Coord::new(3, 0)));
        assert_eq!(w.bots[worker].carrying.count(Item::Wood), 20);
        next_step(&mut w);
        refused(w.gather(worker, Dir::Down), "carrying all it can");
    }

    #[test]
    fn put_and_take_move_items_between_units_storage_and_the_ground() {
        let (mut w, commander, worker, enemy) = world();
        w.bots[worker].carrying.add(Item::Wood, 5);
        // Onto the ground, then back.
        assert_eq!(w.put(worker, Dir::Down, Item::Wood, 3).unwrap(), 3);
        assert_eq!(w.piles[&(1, 2)].count(Item::Wood), 3);
        next_step(&mut w);
        assert_eq!(w.take(worker, Dir::Down, Item::Wood, 10).unwrap(), 3);
        assert!(!w.piles.contains_key(&(1, 2)));
        assert!(w.changes.piles.contains(&Coord::new(2, 1)));
        // Into the base's storage: the worker moves next to it first.
        next_step(&mut w);
        w.move_unit(worker, Dir::Down).unwrap();
        assert_eq!(w.put(worker, Dir::Left, Item::Wood, 5).unwrap(), 5);
        assert_eq!(w.base(A).stored.count(Item::Wood), START.1 + 5);
        // To a teammate, not to or from the other team.
        next_step(&mut w);
        w.move_unit(worker, Dir::Down).unwrap();
        w.bots[worker].carrying.add(Item::Stone, 2);
        assert_eq!(w.put(worker, Dir::Left, Item::Stone, 2).unwrap(), 2);
        assert_eq!(w.bots[commander].carrying.count(Item::Stone), 2);
        next_step(&mut w);
        w.move_unit(enemy, Dir::Down).unwrap();
        next_step(&mut w);
        w.move_unit(enemy, Dir::Left).unwrap();
        w.bots[enemy].carrying.add(Item::Wood, 1);
        w.bots[worker].carrying.add(Item::Stone, 1);
        refused(w.take(worker, Dir::Right, Item::Wood, 1), "another team");
        refused(w.put(worker, Dir::Right, Item::Stone, 1), "another team");
    }

    #[test]
    fn buildings_cost_what_the_builder_carries_and_take_work_to_finish() {
        let (mut w, _, worker, enemy) = world();
        refused(w.build(worker, Dir::Down, Building::Chest), "not carrying");
        refused(w.build(worker, Dir::Down, Building::Base), "can't be built");
        w.bots[worker].carrying.add(Item::Wood, 12);
        assert_eq!(w.build(worker, Dir::Down, Building::Chest).unwrap(), 4);
        assert_eq!(w.bots[worker].carrying.count(Item::Wood), 2);
        let chest = w.structure_on(Coord::new(2, 1)).unwrap();
        // Unfinished, it stores nothing and blocks the way.
        next_step(&mut w);
        refused(
            w.put(worker, Dir::Down, Item::Wood, 1),
            "not a storage building",
        );
        refused(w.move_unit(worker, Dir::Down), "blocked");
        for left in (0..4).rev() {
            next_step(&mut w);
            assert_eq!(w.build(worker, Dir::Down, Building::Chest).unwrap(), left);
        }
        next_step(&mut w);
        refused(
            w.build(worker, Dir::Down, Building::Chest),
            "another building",
        );
        assert_eq!(w.put(worker, Dir::Down, Item::Wood, 2).unwrap(), 2);
        assert_eq!(w.structures[&chest].stored.count(Item::Wood), 2);
        // Not the other team's to use.
        next_step(&mut w);
        w.move_unit(enemy, Dir::Left).unwrap();
        refused(
            w.take(enemy, Dir::Left, Item::Wood, 1),
            "not a storage building of your team",
        );
    }

    #[test]
    fn the_commander_spawns_next_to_the_base_for_wood_up_to_the_cap() {
        let (mut w, commander, worker, _) = world();
        refused(w.spawn(worker, 10), "only a commander");
        refused(w.spawn(commander, 2), "unit cap");
        let id = w.spawn(commander, 10).unwrap();
        assert_eq!(w.bots[BotId(id)].kind, Kind::Worker);
        assert!(w.bots[BotId(id)].pos.dist(Coord::new(1, 1)) == 1);
        assert_eq!(w.base(A).stored.count(Item::Wood), START.1 - SPAWN_COST.1);
        next_step(&mut w);
        w.move_unit(commander, Dir::Down).unwrap();
        next_step(&mut w);
        refused(w.spawn(commander, 10), "next to the base");
        next_step(&mut w);
        w.move_unit(commander, Dir::Up).unwrap();
        let base = w.bases[0];
        w.structures.get_mut(&base).unwrap().stored = Items::default();
        refused(w.spawn(commander, 10), "can't pay");
    }

    #[test]
    fn say_reaches_teammates_in_range_or_on_the_radio_network() {
        let (mut w, commander, worker, enemy) = world();
        w.say(worker, b"hi".to_vec()).unwrap();
        refused(w.say(worker, b"again".to_vec()), "already said");
        next_step(&mut w);
        refused(w.say(worker, vec![0; MESSAGE_BYTES + 1]), "longer");
        let said = &w.said[0];
        assert!(w.hears(commander, said));
        assert!(!w.hears(worker, said), "not yourself");
        assert!(!w.hears(enemy, said), "not the other team");
        // Out of a unit's own range, but both within the base's radio range of 24.
        let far = Said {
            team: A,
            from: worker,
            at: Coord::new(1, 2 + SAY_RANGE + 2),
            data: vec![],
        };
        assert!(w.hears(commander, &far));
        let off = Said {
            team: A,
            from: worker,
            at: Coord::new(1, 1 + 30),
            data: vec![],
        };
        assert!(!w.hears(commander, &off));
    }

    #[test]
    fn a_pile_s_snapshot_rows_replace_what_was_on_its_tile() {
        let (mut w, _, worker, _) = world();
        let rows = |w: &World| w.snapshot(1, false).piles;
        w.bots[worker].carrying.add(Item::Wood, 2);
        w.bots[worker].carrying.add(Item::Coal, 1);
        w.put(worker, Dir::Down, Item::Wood, 2).unwrap();
        next_step(&mut w);
        w.put(worker, Dir::Down, Item::Coal, 1).unwrap();
        let (wood, coal) = (Item::Wood as u64, Item::Coal as u64);
        // Items in id order: coal before wood.
        assert_eq!(rows(&w), vec![2, 1, coal, 1, 2, 1, wood, 2]);
        w.changes = Changes::default();
        next_step(&mut w);
        w.take(worker, Dir::Down, Item::Coal, 1).unwrap();
        assert_eq!(rows(&w), vec![2, 1, wood, 2]);
        next_step(&mut w);
        w.take(worker, Dir::Down, Item::Wood, 2).unwrap();
        assert_eq!(rows(&w), vec![2, 1, 0, 0]);
        // Nothing can be built on a tile with things on it.
        next_step(&mut w);
        w.put(worker, Dir::Down, Item::Wood, 1).unwrap();
        next_step(&mut w);
        w.bots[worker].carrying.add(Item::Wood, 4);
        refused(
            w.build(worker, Dir::Down, Building::WoodWall),
            "isn't clear",
        );
    }

    #[test]
    fn signs_replace_and_come_down() {
        let (mut w, _, worker, _) = world();
        w.mark(worker, b"wood east".to_vec()).unwrap();
        next_step(&mut w);
        w.mark(worker, b"gone".to_vec()).unwrap();
        assert_eq!(w.signs[&(0, 2)], b"gone");
        next_step(&mut w);
        w.mark(worker, vec![]).unwrap();
        assert!(w.signs.is_empty());
        assert!(w.changes.signs.contains(&Coord::new(2, 0)));
    }

    #[test]
    fn a_removed_unit_leaves_what_it_carried() {
        let (mut w, _, worker, _) = world();
        w.bots[worker].carrying.add(Item::Coal, 3);
        w.remove_unit(worker);
        assert_eq!(w.piles[&(0, 2)].count(Item::Coal), 3);
        assert_eq!(w.unit_on(Coord::new(2, 0)), None);
        assert_eq!(w.changes.removed, vec![worker.0]);
    }
}
