//! Each action checks everything before it changes anything, so a refused action
//! leaves the set as it was.

use ucbc_engine::{ActionError, BotId};

use crate::coord::Coord;
use crate::game::Ucbc2027;
use crate::map::Environment;
use crate::rules;
use crate::state::Item;
use crate::unit::{Dino, Unit};
use crate::view::{Dropped, ItemView, Spawned, UnitView};

fn invalid(message: &str) -> ActionError {
    ActionError::Invalid(message.into())
}

impl Ucbc2027 {
    pub(crate) fn spawn(&mut self, bot: BotId, at: Coord) -> Result<Spawned, ActionError> {
        let team = self.bots[bot].team();
        let lab = self.bots[bot]
            .as_lab()
            .ok_or_else(|| invalid("only a lab can spawn"))?;
        if self.turn.spawned {
            return Err(invalid("already spawned this turn"));
        }
        if !lab.borders(at) {
            return Err(invalid("must spawn next to the lab"));
        }
        if !self.state.is_free(at) {
            return Err(invalid("the tile is not free"));
        }
        if self.state.team(team).bones < rules::SPAWN_COST {
            return Err(invalid("not enough bones"));
        }
        self.state.team_mut(team).bones -= rules::SPAWN_COST;
        let id = self.bots.spawn(team, Unit::Dino(Dino::new(at)));
        self.state.items[at] = Some(Item::Dino(id));
        self.turn.spawned = true;
        Ok(Spawned { bot_id: id.0, at })
    }

    pub(crate) fn move_to(&mut self, bot: BotId, to: Coord) -> Result<Coord, ActionError> {
        let dino = self.bots[bot]
            .as_dino_mut()
            .ok_or_else(|| invalid("only a dino can move"))?;
        if self.turn.moved {
            return Err(invalid("already moved this turn"));
        }
        if dino.pos.dist(to) > dino.stats().move_range {
            return Err(invalid("out of range"));
        }
        if self.state.map.env(to).is_none() {
            return Err(invalid("off the board"));
        }
        if !self.state.is_free(to) {
            return Err(invalid("the tile is not free"));
        }
        self.state.items[to] = self.state.items[dino.pos].take();
        dino.pos = to;
        self.turn.moved = true;
        Ok(to)
    }

    pub(crate) fn grab(&mut self, bot: BotId, at: Coord) -> Result<ItemView, ActionError> {
        let dino = self.bots[bot]
            .as_dino_mut()
            .ok_or_else(|| invalid("only a dino can grab"))?;
        if dino.held.is_some() {
            return Err(invalid("already holding something"));
        }
        if dino.pos.dist(at) > dino.stats().action_range {
            return Err(invalid("out of range"));
        }
        let item = match self.state.items.get(at).copied() {
            None => return Err(invalid("off the board")),
            Some(None) => return Err(invalid("nothing there")),
            Some(Some(item)) if !item.carriable() => return Err(invalid("cannot carry that")),
            Some(Some(item)) => item,
        };
        self.state.items[at] = None;
        dino.held = Some(item);
        Ok(self.item_view(item))
    }

    pub(crate) fn drop(&mut self, bot: BotId, at: Coord) -> Result<Dropped, ActionError> {
        let team = self.bots[bot].team();
        let dino = self.bots[bot]
            .as_dino_mut()
            .ok_or_else(|| invalid("only a dino can drop"))?;
        let Some(item) = dino.held else {
            return Err(invalid("holding nothing"));
        };
        if dino.pos.dist(at) > dino.stats().action_range {
            return Err(invalid("out of range"));
        }
        let deposit = match self.state.map.env(at) {
            None => return Err(invalid("off the board")),
            Some(Environment::Lab { team: owner }) if owner != team.0 => {
                return Err(invalid("not your lab"));
            }
            Some(Environment::Lab { .. }) if item != Item::Fossil => {
                return Err(invalid("only fossils go in the lab"));
            }
            Some(Environment::Lab { .. }) => true,
            Some(_) if !self.state.is_free(at) => return Err(invalid("the tile is not free")),
            Some(_) => false,
        };
        dino.held = None;
        if deposit {
            let team = self.state.team_mut(team);
            team.fossils += 1;
            Ok(Dropped::Deposited {
                fossils: team.fossils,
            })
        } else {
            self.state.items[at] = Some(item);
            Ok(Dropped::Placed)
        }
    }

    /// Why `bot` may not merge with `target` now, if it may not. The merge action and
    /// the `can_merge` query both ask this.
    pub(crate) fn check_merge(&self, bot: BotId, target: BotId) -> Result<(), ActionError> {
        let team = self.bots[bot].team();
        let dino = self.bots[bot]
            .as_dino()
            .ok_or_else(|| invalid("only a dino can merge"))?;
        if self.turn.merges >= rules::MERGES_PER_TURN {
            return Err(invalid("already merged this turn"));
        }
        if target == bot {
            return Err(invalid("cannot merge with itself"));
        }
        let other = self
            .bots
            .get(target)
            .ok_or_else(|| invalid("no such bot"))?;
        if other.team() != team {
            return Err(invalid("not your team"));
        }
        let other = other
            .as_dino()
            .ok_or_else(|| invalid("can only merge with a dino"))?;
        if dino.pos.dist(other.pos) > dino.stats().action_range {
            return Err(invalid("out of range"));
        }
        Ok(())
    }

    /// The target's bot goes; this one takes its tile. If the target held something,
    /// this dino takes it and leaves what it held on its old tile.
    pub(crate) fn merge(&mut self, bot: BotId, target: BotId) -> Result<UnitView, ActionError> {
        self.check_merge(bot, target)?;
        let other = self.bots[target].as_dino().expect("checked above");
        let (at, level, health, held) = (other.pos, other.level, other.health, other.held);
        self.bots.remove(target);
        let dino = self.bots[bot].as_dino_mut().expect("checked above");
        let from = dino.pos;
        dino.pos = at;
        dino.level += level;
        dino.health = (dino.health + health).min(rules::DINO_MAX_HEALTH);
        let left = match held {
            Some(_) => std::mem::replace(&mut dino.held, held),
            None => None,
        };
        self.state.items[from] = left;
        self.state.items[at] = Some(Item::Dino(bot));
        self.turn.merges += 1;
        Ok(self.unit_view(bot))
    }

    /// Why `bot` may not attack `target` now, if it may not. The attack action and
    /// the `can_attack` query both ask this.
    pub(crate) fn check_attack(&self, bot: BotId, target: BotId) -> Result<(), ActionError> {
        let team = self.bots[bot].team();
        let dino = self.bots[bot]
            .as_dino()
            .ok_or_else(|| invalid("only a dino can attack"))?;
        if self.turn.attacks >= rules::ATTACKS_PER_TURN {
            return Err(invalid("already attacked this turn"));
        }
        let other = self
            .bots
            .get(target)
            .ok_or_else(|| invalid("no such bot"))?;
        if other.team() == team {
            return Err(invalid("cannot attack your own team"));
        }
        let other = other
            .as_dino()
            .ok_or_else(|| invalid("can only attack a dino"))?;
        if dino.pos.dist(other.pos) > dino.stats().attack_range {
            return Err(invalid("out of range"));
        }
        if self.state.team(team).bones < rules::ATTACK_COST {
            return Err(invalid("not enough bones"));
        }
        Ok(())
    }

    /// A target left with no health is removed, its tile left with what it held.
    pub(crate) fn attack(&mut self, bot: BotId, target: BotId) -> Result<(), ActionError> {
        self.check_attack(bot, target)?;
        let team = self.bots[bot].team();
        let level = |id| self.bots[id].as_dino().expect("checked above").level;
        let damage = rules::damage(level(bot), level(target));
        self.state.team_mut(team).bones -= rules::ATTACK_COST;
        self.turn.attacks += 1;
        let other = self.bots[target].as_dino_mut().expect("checked above");
        other.health = other.health.saturating_sub(damage);
        if other.health == 0 {
            self.state.clear_dino(other);
            self.bots.remove(target);
        }
        Ok(())
    }
}
