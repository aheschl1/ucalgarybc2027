use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One team, one code submission. Fixed for the whole match.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct TeamId(pub u32);

impl std::fmt::Display for TeamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One unit the game asks to step. Ids are assigned by the game, unique within a set.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct BotId(pub u64);

impl std::fmt::Display for BotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A bot and the team that owns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct BotRef {
    pub id: BotId,
    pub team: TeamId,
}

impl BotRef {
    pub fn new(id: u64, team: u32) -> Self {
        Self {
            id: BotId(id),
            team: TeamId(team),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TeamInfo {
    pub id: TeamId,
    pub name: String,
}

impl TeamInfo {
    pub fn new(id: TeamId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}
