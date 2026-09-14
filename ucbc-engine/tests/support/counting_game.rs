//! "First team to the target": every tick each live bot, teams interleaved, increments
//! its team's counter by 1 to 3. A team with no bots left is out; the last team
//! standing wins by forfeit.
//!
//! `game_config`: `{"bots_per_team": n, "target": t}` (defaults 1 and 5). Bot ids are
//! `team + teams * k` for the k-th bot of a team.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::json;
use ucbc_engine::{
    ActionError, BotFailure, BotId, BotRef, EngineError, Game, GameStatus, Outcome, QueryError,
    SetSetup, TeamId,
};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Increment { by: u32 },
}

#[derive(Serialize)]
pub struct Incremented {
    pub count: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Query {
    Counts,
}

#[derive(Serialize)]
pub struct Counts {
    pub counts: Vec<u32>,
    pub you: TeamId,
    pub bot: BotId,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub counts: Vec<u32>,
    pub bots: Vec<BotRef>,
}

pub struct CountingGame {
    target: u32,
    counts: Vec<u32>,
    /// Live bots in step order.
    order: Vec<BotRef>,
    acted: bool,
    dead: Vec<BotId>,
    status: GameStatus,
}

impl Game for CountingGame {
    const NAME: &'static str = "counting";
    type Query = Query;
    type QueryResponse = Counts;
    type Action = Action;
    type ActionResponse = Incremented;
    type Snapshot = Snapshot;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        let cfg = setup.game_config.clone().unwrap_or(json!({}));
        let bots_per_team = cfg["bots_per_team"].as_u64().unwrap_or(1) as u32;
        let target = cfg["target"].as_u64().unwrap_or(5) as u32;
        let teams = setup.teams;
        let mut order = Vec::new();
        for k in 0..bots_per_team {
            for i in 0..teams {
                let team = (setup.first_team.0 + i) % teams;
                order.push(BotRef::new(u64::from(team + teams * k), team));
            }
        }
        Ok(CountingGame {
            target,
            counts: vec![0; teams as usize],
            order,
            acted: false,
            dead: Vec::new(),
            status: GameStatus::InProgress,
        })
    }

    fn schedule(&mut self) -> Vec<BotRef> {
        self.order.clone()
    }

    fn handle_query(&self, bot: BotRef, query: Query) -> Result<Counts, QueryError> {
        match query {
            Query::Counts => Ok(Counts {
                counts: self.counts.clone(),
                you: bot.team,
                bot: bot.id,
            }),
        }
    }

    fn apply_action(&mut self, bot: BotRef, action: Action) -> Result<Incremented, ActionError> {
        if self.acted {
            return Err(ActionError::Invalid("already acted this step".into()));
        }
        let Action::Increment { by } = action;
        if !(1..=3).contains(&by) {
            return Err(ActionError::Invalid(format!(
                "increment must be 1..=3, got {by}"
            )));
        }
        let team = bot.team.0 as usize;
        self.counts[team] += by;
        self.acted = true;
        if self.counts[team] >= self.target {
            self.status = GameStatus::Complete(Outcome::win(bot.team));
        }
        Ok(Incremented {
            count: self.counts[team],
        })
    }

    fn end_step(&mut self, _bot: BotRef) {
        self.acted = false;
    }

    fn bot_failed(&mut self, bot: BotRef, failure: &BotFailure) {
        self.acted = false;
        self.order.retain(|b| *b != bot);
        self.dead.push(bot.id);
        let alive: BTreeSet<TeamId> = self.order.iter().map(|b| b.team).collect();
        let detail = format!("team {} has no bots left: {}", bot.team, failure.detail());
        match alive.iter().next() {
            Some(&winner) if alive.len() == 1 => {
                self.status = GameStatus::Complete(Outcome::forfeit(winner, detail));
            }
            None => self.status = GameStatus::Complete(Outcome::draw(detail)),
            _ => {}
        }
    }

    fn end_tick(&mut self) {}

    fn despawned(&mut self) -> Vec<BotId> {
        std::mem::take(&mut self.dead)
    }

    fn status(&self) -> GameStatus {
        self.status.clone()
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            counts: self.counts.clone(),
            bots: self.order.clone(),
        }
    }
}
