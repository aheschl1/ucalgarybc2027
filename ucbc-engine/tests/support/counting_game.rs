//! "First team to the target": every tick each live bot, teams interleaved, increments
//! its team's counter by 1 to 3. A team with no bots left is out; the last team
//! standing wins by forfeit.
//!
//! `game_config`: `{"bots_per_team": n, "target": t}` (defaults 1 and 5). Bots spawn
//! one per team in step order, `bots_per_team` times, so ids interleave the teams. With
//! `"leader_wins_at_limit": true`, a set at the tick limit goes to the highest count
//! instead of a draw. With `"schedule_unknown_after_first_tick": true`, the game
//! schedules a bot it does not have from tick 1.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, Outcome,
    QueryError, Request, SetSetup, TeamId,
};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Increment(Request<By, Incremented>),
}

#[derive(Deserialize, JsonSchema)]
pub struct By {
    pub by: u32,
}

#[derive(Serialize, JsonSchema)]
pub struct Incremented {
    pub count: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    Counts(Request<(), Counts>),
}

#[derive(Serialize, JsonSchema)]
pub struct Counts {
    pub counts: Vec<u32>,
    pub you: TeamId,
    pub bot: BotId,
}

#[derive(Serialize, JsonSchema)]
pub struct Snapshot {
    pub counts: Vec<u32>,
    pub bots: Vec<BotId>,
}

pub struct CountingGame {
    bots: BotManager<()>,
    target: u32,
    counts: Vec<u32>,
    acted: bool,
    schedule_unknown_after_first_tick: bool,
    ticks: u32,
    leader_wins_at_limit: bool,
    status: GameStatus,
}

impl Game for CountingGame {
    const NAME: &'static str = "counting";
    type Query = Queries;
    type Action = Action;
    type Snapshot = Snapshot;
    type Bot = ();

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        let cfg = setup.game_config.clone().unwrap_or(json!({}));
        let bots_per_team = cfg["bots_per_team"].as_u64().unwrap_or(1) as u32;
        let target = cfg["target"].as_u64().unwrap_or(5) as u32;
        let teams = setup.teams.len() as u32;
        let mut bots = BotManager::new();
        for _ in 0..bots_per_team {
            for &team in &setup.teams {
                bots.spawn(team, ());
            }
        }
        Ok(CountingGame {
            bots,
            target,
            counts: vec![0; teams as usize],
            acted: false,
            schedule_unknown_after_first_tick: cfg["schedule_unknown_after_first_tick"] == true,
            ticks: 0,
            leader_wins_at_limit: cfg["leader_wins_at_limit"] == true,
            status: GameStatus::InProgress,
        })
    }

    fn bots(&self) -> &BotManager<()> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<()> {
        &mut self.bots
    }

    fn schedule(&mut self) -> Vec<BotId> {
        let mut ids = self.bots.ids();
        if self.schedule_unknown_after_first_tick && self.ticks > 0 {
            ids.push(BotId(999));
        }
        ids
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        let counts = Counts {
            counts: self.counts.clone(),
            you: self.bots[bot].team(),
            bot,
        };
        let Queries::Counts(q) = query;
        Ok(q.reply(counts))
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        if self.acted {
            return Err(ActionError::Invalid("already acted this step".into()));
        }
        let Action::Increment(a) = action;
        let by = a.by;
        if !(1..=3).contains(&by) {
            return Err(ActionError::Invalid(format!(
                "increment must be 1..=3, got {by}"
            )));
        }
        let team_id = self.bots[bot].team();
        let team = team_id.0 as usize;
        self.counts[team] += by;
        self.acted = true;
        if self.counts[team] >= self.target {
            self.status = GameStatus::Complete(Outcome::win(team_id));
        }
        Ok(a.reply(Incremented {
            count: self.counts[team],
        }))
    }

    fn end_step(&mut self, _bot: BotId) {
        self.acted = false;
    }

    /// The engine removes the bot after this, so it does not count as alive.
    fn bot_failed(&mut self, bot: BotId, failure: &BotFailure) {
        self.acted = false;
        let alive: BTreeSet<TeamId> = self
            .bots
            .ids()
            .into_iter()
            .filter(|&id| id != bot)
            .map(|id| self.bots[id].team())
            .collect();
        let team = self.bots[bot].team();
        let detail = format!("team {team} has no bots left: {}", failure.detail());
        match alive.iter().next() {
            Some(&winner) if alive.len() == 1 => {
                self.status = GameStatus::Complete(Outcome::forfeit(winner, detail));
            }
            None => self.status = GameStatus::Complete(Outcome::draw(detail)),
            _ => {}
        }
    }

    fn end_tick(&mut self) {
        self.ticks += 1;
    }

    fn status(&self) -> GameStatus {
        self.status.clone()
    }

    fn tick_limit(&self, max_ticks: u32) -> Outcome {
        let best = self.counts.iter().max().copied().unwrap_or(0);
        let leaders: Vec<usize> = (0..self.counts.len())
            .filter(|&t| self.counts[t] == best)
            .collect();
        match leaders[..] {
            [leader] if self.leader_wins_at_limit => Outcome::win(TeamId(leader as u32)),
            _ => Outcome::draw(format!("tick limit of {max_ticks} reached")),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            counts: self.counts.clone(),
            bots: self.bots.ids(),
        }
    }
}
