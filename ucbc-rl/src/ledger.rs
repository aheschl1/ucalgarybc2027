use std::collections::HashMap;

use ucbc_engine::{BotId, TeamId, TickMoment};

/// What a unit earned between two of its turns
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reward {
    /// The score delta across the unit's own step
    pub own: f32,
    /// The change from any cause, its own step included.
    pub team: f32,
    /// Once the set is over, for units still in it: 1 won, -1 lost, 0 drawn.
    pub result: f32,
}

struct Mark {
    team: TeamId,
    /// The team's lead at the unit's last turn.
    lead: f32,
    own: f32,
}

/// One set's rewards, fed by the runner's tick hook.
#[derive(Default)]
pub(crate) struct Ledger {
    leads: Vec<f32>,
    marks: HashMap<BotId, Mark>,
    winner: Option<Option<TeamId>>,
}

impl Ledger {
    /// The first tick after a set's end starts the next set's.
    pub fn tick(&mut self, moment: TickMoment, scores: &[i64]) {
        if self.winner.is_some() {
            *self = Self::default();
        }
        let total: i64 = scores.iter().sum();
        let others = (scores.len().max(2) - 1) as f32;
        let leads: Vec<f32> = scores
            .iter()
            .map(|&s| s as f32 - (total - s) as f32 / others)
            .collect();
        if let TickMoment::Step { bot, team } = moment
            && let Some(mark) = self.marks.get_mut(&bot)
        {
            let t = team.0 as usize;
            mark.own += leads[t] - self.leads[t];
        }
        self.leads = leads;
    }

    /// What `bot` earned since its last turn; it starts counting again from here.
    pub fn turn(&mut self, bot: BotId, team: TeamId) -> Reward {
        let lead = self.lead(team);
        let mark = self.marks.entry(bot).or_insert(Mark {
            team,
            lead,
            own: 0.0,
        });
        let reward = Reward {
            own: mark.own,
            team: lead - mark.lead,
            result: 0.0,
        };
        (mark.lead, mark.own) = (lead, 0.0);
        reward
    }

    /// What `bot` earned since its last turn, with the set's result if the set is over.
    pub fn gone(&mut self, bot: BotId) -> Reward {
        let Some(mark) = self.marks.remove(&bot) else {
            return Reward::default();
        };
        let result = match self.winner {
            Some(Some(team)) if team == mark.team => 1.0,
            Some(Some(_)) => -1.0,
            _ => 0.0,
        };
        Reward {
            own: mark.own,
            team: self.lead(mark.team) - mark.lead,
            result,
        }
    }

    pub fn end(&mut self, winner: Option<TeamId>) {
        self.winner = Some(winner);
    }

    fn lead(&self, team: TeamId) -> f32 {
        self.leads[team.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: TeamId = TeamId(0);
    const B: TeamId = TeamId(1);

    fn step(bot: u64, team: TeamId) -> TickMoment {
        TickMoment::Step {
            bot: BotId(bot),
            team,
        }
    }

    #[test]
    fn a_unit_owns_its_step_and_shares_the_rest() {
        let mut l = Ledger::default();
        l.tick(TickMoment::TickStart, &[10, 10]);
        assert_eq!(l.turn(BotId(0), A), Reward::default());
        l.tick(step(0, A), &[15, 10]);
        assert_eq!(l.turn(BotId(1), B), Reward::default());
        l.tick(step(1, B), &[15, 12]);
        // Between turns, a job finishes for A: no one's step.
        l.tick(TickMoment::TickStart, &[18, 12]);
        let r = l.turn(BotId(0), A);
        assert_eq!((r.own, r.team), (5.0, 6.0));
        let r = l.turn(BotId(1), B);
        assert_eq!((r.own, r.team), (2.0, -1.0));
    }

    #[test]
    fn units_still_in_at_the_end_get_the_result() {
        let mut l = Ledger::default();
        l.tick(TickMoment::TickStart, &[0, 0]);
        l.turn(BotId(0), A);
        l.turn(BotId(1), B);
        l.turn(BotId(2), A);
        l.tick(step(2, A), &[0, 5]);
        // Killed mid-set: no result.
        assert_eq!(l.gone(BotId(2)).result, 0.0);
        l.end(Some(A));
        let (a, b) = (l.gone(BotId(0)), l.gone(BotId(1)));
        assert_eq!((a.result, a.team), (1.0, -5.0));
        assert_eq!((b.result, b.team), (-1.0, 5.0));
        assert_eq!(l.gone(BotId(9)), Reward::default());
    }
}
