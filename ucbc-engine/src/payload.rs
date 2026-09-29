//! Queries, actions, and snapshots are JSON objects with a `"type"` discriminator.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::DecodeError;

pub fn kind(payload: &Value) -> Option<&str> {
    payload.get("type").and_then(Value::as_str)
}

/// Deserialize a tagged payload into a `#[serde(tag = "type")]` enum.
pub fn decode<T: DeserializeOwned>(payload: &Value) -> Result<T, DecodeError> {
    let Some(kind) = kind(payload) else {
        return Err(DecodeError::Malformed("missing `type` field".into()));
    };
    serde_json::from_value(payload.clone()).map_err(|e| {
        if e.to_string().starts_with("unknown variant") {
            DecodeError::UnknownType(kind.to_string())
        } else {
            DecodeError::Malformed(e.to_string())
        }
    })
}

/// Plain data types always serialize; a failure here is a bug in the game's types.
pub(crate) fn encode<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("game response serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Action {
        Place { row: u32, col: u32 },
    }

    #[test]
    fn decodes_known_action() {
        let a: Action = decode(&json!({"type": "place", "row": 1, "col": 2})).unwrap();
        assert_eq!(a, Action::Place { row: 1, col: 2 });
    }

    #[test]
    fn maps_errors() {
        assert_eq!(
            decode::<Action>(&json!({"type": "jump"})),
            Err(DecodeError::UnknownType("jump".into()))
        );
        assert!(matches!(
            decode::<Action>(&json!({"type": "place", "row": "x"})),
            Err(DecodeError::Malformed(_))
        ));
        assert!(matches!(
            decode::<Action>(&json!({"row": 1})),
            Err(DecodeError::Malformed(_))
        ));
    }
}
