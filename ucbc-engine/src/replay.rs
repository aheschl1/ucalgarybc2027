//! The replay format, written as compact JSON, gzipped when the file name ends in `.gz`.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use flate2::Compression;
use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bot::BotResourceLimit;
use crate::error::EngineError;
use crate::game::Outcome;
use crate::ids::{BotId, TeamId, TeamInfo};

pub const DEFAULT_MAX_TICKS: u32 = 1000;

fn default_max_ticks() -> u32 {
    DEFAULT_MAX_TICKS
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MatchConfig {
    /// Registry key of the game, e.g. `"tictactoe"`.
    pub game: String,
    /// Number of sets. The first team rotates each set.
    pub sets: u32,
    pub seed: u64,
    /// Number of teams. Must equal the number of team specs.
    pub teams: u32,
    /// A set still in progress after this many ticks ends as a draw.
    #[serde(default = "default_max_ticks")]
    pub max_ticks: u32,
    /// Opaque to the engine; handed to the game factory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_config: Option<Value>,
    /// What each bot may use.
    pub limits: BotResourceLimit,
}

impl MatchConfig {
    pub fn new(
        game: impl Into<String>,
        sets: u32,
        seed: u64,
        teams: u32,
        limits: BotResourceLimit,
    ) -> Self {
        Self {
            game: game.into(),
            sets,
            seed,
            teams,
            max_ticks: DEFAULT_MAX_TICKS,
            game_config: None,
            limits,
        }
    }

    pub fn max_ticks(mut self, max_ticks: u32) -> Self {
        self.max_ticks = max_ticks;
        self
    }

    pub fn game_config(mut self, game_config: Value) -> Self {
        self.game_config = Some(game_config);
        self
    }
}

/// How a set ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Win,
    Draw,
    Forfeit,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Reason::Win => "win",
            Reason::Draw => "draw",
            Reason::Forfeit => "forfeit",
        })
    }
}

/// Attached to the step where a bot's runtime failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FailureRecord {
    pub kind: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub traceback: String,
}

impl FailureRecord {
    pub fn new(
        kind: impl Into<String>,
        message: impl Into<String>,
        traceback: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            traceback: traceback.into(),
        }
    }
}

/// What a step used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    /// What the bot's runtime charged the step. For Python bots that is instructions
    /// at a nanosecond each, the same count on every machine.
    pub time_us: u64,
    /// Bytes the bot's runtime held after the step; absent when the runtime cannot tell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
}

impl Usage {
    pub fn new(time: Duration, memory: Option<u64>) -> Self {
        Self {
            time_us: time.as_micros() as u64,
            memory,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    pub bot: BotId,
    pub team: TeamId,
    /// Accepted actions only, in order.
    pub actions: Vec<Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<FailureRecord>,
    pub usage: Usage,
}

impl Step {
    pub fn new(
        bot: BotId,
        team: TeamId,
        actions: Vec<Value>,
        stdout: String,
        failure: Option<FailureRecord>,
        usage: Usage,
    ) -> Self {
        Self {
            bot,
            team,
            actions,
            stdout,
            failure,
            usage,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Tick {
    /// 0-based within the set.
    pub number: u32,
    pub steps: Vec<Step>,
    /// Game snapshot after the tick.
    pub state_after: Value,
}

impl Tick {
    pub fn new(number: u32, steps: Vec<Step>, state_after: Value) -> Self {
        Self {
            number,
            steps,
            state_after,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetResult {
    pub index: u32,
    pub first_team: TeamId,
    /// `None` is a draw.
    pub winner_team: Option<TeamId>,
    pub reason: Reason,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Number of ticks played.
    pub ticks: u32,
}

impl SetResult {
    pub fn new(index: u32, first_team: TeamId, outcome: Outcome, ticks: u32) -> Self {
        Self {
            index,
            first_team,
            winner_team: outcome.winner,
            reason: outcome.reason,
            detail: outcome.detail,
            ticks,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetReplay {
    pub index: u32,
    pub first_team: TeamId,
    pub initial_state: Value,
    pub ticks: Vec<Tick>,
    pub result: SetResult,
}

impl SetReplay {
    pub fn new(initial_state: Value, ticks: Vec<Tick>, result: SetResult) -> Self {
        Self {
            index: result.index,
            first_team: result.first_team,
            initial_state,
            ticks,
            result,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatchResult {
    pub sets: Vec<SetResult>,
    /// Sets won, indexed by team.
    pub set_wins: Vec<u32>,
    /// `None` when the top score is shared.
    pub winner_team: Option<TeamId>,
}

impl MatchResult {
    pub fn from_sets(sets: Vec<SetResult>, teams: u32) -> Self {
        let mut set_wins = vec![0u32; teams as usize];
        for set in &sets {
            if let Some(w) = set.winner_team {
                set_wins[w.0 as usize] += 1;
            }
        }
        let best = set_wins.iter().copied().max().unwrap_or(0);
        let leaders: Vec<usize> = set_wins
            .iter()
            .enumerate()
            .filter(|(_, w)| **w == best)
            .map(|(i, _)| i)
            .collect();
        let winner_team = match leaders.as_slice() {
            [only] if best > 0 => Some(TeamId(*only as u32)),
            _ => None,
        };
        Self {
            sets,
            set_wins,
            winner_team,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Replay {
    pub match_id: String,
    pub engine_version: String,
    pub config: MatchConfig,
    pub teams: Vec<TeamInfo>,
    pub sets: Vec<SetReplay>,
    pub result: MatchResult,
}

impl Replay {
    pub fn new(
        match_id: String,
        config: MatchConfig,
        teams: Vec<TeamInfo>,
        sets: Vec<SetReplay>,
    ) -> Self {
        let results = sets.iter().map(|s| s.result.clone()).collect();
        let result = MatchResult::from_sets(results, config.teams);
        Self {
            match_id,
            engine_version: crate::ENGINE_VERSION.to_string(),
            config,
            teams,
            sets,
            result,
        }
    }
}

/// Pretty JSON with a trailing newline; deterministic for equal input.
/// The first bytes of every gzip file.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Compact JSON, gzipped if `path` ends in `.gz`.
pub fn write_replay(path: &Path, replay: &Replay) -> Result<(), EngineError> {
    let file = BufWriter::new(File::create(path)?);
    if path.extension().is_some_and(|e| e == "gz") {
        // Fast: a replay is large, and gzip's higher levels gain little on it.
        let mut gz = GzEncoder::new(file, Compression::fast());
        serde_json::to_writer(&mut gz, replay)?;
        gz.finish()?.flush()?;
    } else {
        let mut w = file;
        serde_json::to_writer(&mut w, replay)?;
        w.write_all(b"\n")?;
        w.flush()?;
    }
    Ok(())
}

/// Reads a replay, gzipped or not, whatever the file is called.
pub fn read_replay(path: &Path) -> Result<Replay, EngineError> {
    let mut r = BufReader::new(File::open(path)?);
    if r.fill_buf()?.starts_with(&GZIP_MAGIC) {
        Ok(serde_json::from_reader(GzDecoder::new(r))?)
    } else {
        Ok(serde_json::from_reader(r)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(index: u32, winner: Option<u32>) -> SetResult {
        let outcome = match winner {
            Some(w) => Outcome::win(TeamId(w)),
            None => Outcome::draw(""),
        };
        SetResult::new(index, TeamId(index % 2), outcome, 0)
    }

    #[test]
    fn match_result_scores_and_ties() {
        let r = MatchResult::from_sets(vec![set(0, Some(0)), set(1, Some(1)), set(2, Some(0))], 2);
        assert_eq!(r.set_wins, vec![2, 1]);
        assert_eq!(r.winner_team, Some(TeamId(0)));

        let tie = MatchResult::from_sets(vec![set(0, Some(0)), set(1, Some(1))], 2);
        assert_eq!(tie.winner_team, None);

        let all_draws = MatchResult::from_sets(vec![set(0, None), set(1, None)], 2);
        assert_eq!(all_draws.set_wins, vec![0, 0]);
        assert_eq!(all_draws.winner_team, None);
    }

    #[test]
    fn reason_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&Reason::Forfeit).unwrap(),
            "\"forfeit\""
        );
        assert_eq!(Reason::Draw.to_string(), "draw");
    }

    #[test]
    fn config_defaults_max_ticks() {
        let c: MatchConfig = serde_json::from_str(
            r#"{"game":"x","sets":1,"seed":0,"teams":2,"limits":{"step_ms":1,"memory_bytes":1}}"#,
        )
        .unwrap();
        assert_eq!(c.max_ticks, DEFAULT_MAX_TICKS);
    }
}
