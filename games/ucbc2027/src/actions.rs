//! Each action checks everything before it changes anything, so a refused action
//! leaves the set as it was.

use ucbc_engine::{ActionError, BotId};

use crate::coord::Coord;
use crate::game::Ucbc2027;
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
        if !lab.borders(at) {
            return Err(invalid("must spawn next to the lab"));
        }
        if !self.state.is_free(at) {
            return Err(invalid("the tile is not free"));
        }
        let id = self.bots.spawn(team, Unit::Dino(Dino::new(at)));
        self.state.items[at] = Some(Item::Dino(id));
        Ok(Spawned { bot_id: id.0, at })
    }
}
