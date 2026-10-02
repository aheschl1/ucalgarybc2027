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

/// One unit the game asks to step. Handed out by the game's `BotManager`, unique
/// within a set.
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
