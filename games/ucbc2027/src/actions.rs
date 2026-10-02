//! Each action checks everything before it changes anything, so a refused action
//! leaves the set as it was.

use ucbc_engine::{ActionError, BotId};

use crate::coord::Coord;
use crate::game::Ucbc2027;
use crate::rules;
use crate::state::Item;
use crate::unit::{Dino, Unit};
use crate::view::Spawned;

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
        if dino.pos.dist(to) > rules::stats(dino).move_range {
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
}
