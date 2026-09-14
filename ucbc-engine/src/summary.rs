//! Small JSON document written next to the replay; what the match worker reads.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::ids::{TeamId, TeamInfo};
use crate::replay::{Replay, SetResult};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub match_id: String,
    pub engine_version: String,
    pub game: String,
    pub created_at_unix_ms: u64,
    pub teams: Vec<TeamInfo>,
    pub sets: Vec<SetResult>,
    pub set_wins: Vec<u32>,
    pub winner_team: Option<TeamId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_path: Option<String>,
}

pub fn summarize(replay: &Replay, replay_path: Option<&Path>) -> Summary {
    let created_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Summary {
        match_id: replay.match_id.clone(),
        engine_version: replay.engine_version.clone(),
        game: replay.config.game.clone(),
        created_at_unix_ms,
        teams: replay.teams.clone(),
        sets: replay.result.sets.clone(),
        set_wins: replay.result.set_wins.clone(),
        winner_team: replay.result.winner_team,
        replay_path: replay_path.map(|p| p.display().to_string()),
    }
}

pub fn write_summary(path: &Path, summary: &Summary) -> Result<(), EngineError> {
    let mut w = BufWriter::new(File::create(path)?);
    serde_json::to_writer_pretty(&mut w, summary)?;
    w.write_all(b"\n")?;
    w.flush()?;
    Ok(())
}
