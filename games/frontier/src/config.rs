//! `game_config`, every field optional: `{"size": 1024, "units": 100, "vision": 16,
//! "keyframe": 100, "flat": false}`.

use serde::Deserialize;
use serde_json::Value;
use ucbc_engine::EngineError;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The map is `size` x `size`.
    pub size: usize,
    /// Most units a team can have, its commander included.
    pub units: u32,
    /// A unit sees the square `vision` tiles out from it.
    pub vision: usize,
    /// A snapshot lists every unit once every `keyframe` ticks, and only changes between.
    pub keyframe: u32,
    /// Grass everywhere but the bases' starting areas: a simple map to start learning on.
    pub flat: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            size: 1024,
            units: 100,
            vision: 16,
            keyframe: 100,
            flat: false,
        }
    }
}

impl Config {
    pub fn from(value: Option<&Value>) -> Result<Self, EngineError> {
        let bad = |why: String| EngineError::Config(format!("game_config: {why}"));
        let config: Config = match value {
            None => Config::default(),
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| bad(e.to_string()))?,
        };
        if !(64..=4096).contains(&config.size) {
            return Err(bad(format!("size {} is outside 64..=4096", config.size)));
        }
        if config.vision > config.size {
            return Err(bad(format!(
                "vision {} is over size {}",
                config.vision, config.size
            )));
        }
        if config.units == 0 || config.keyframe == 0 {
            return Err(bad("units and keyframe must be at least 1".into()));
        }
        Ok(config)
    }
}
