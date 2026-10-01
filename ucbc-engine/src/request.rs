//! Queries and actions name their reply type, so a game can only answer one with that
//! type and the generated Python API can type each method.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::Deref;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::payload::encode;

/// One variant of a game's query or action enum: parameters `P`, answered with an `R`.
/// Derefs to `P`. Its schema is `P`'s with `R`'s under `x-returns`.
pub struct Request<P, R> {
    params: P,
    reply: PhantomData<fn(R)>,
}

impl<P, R: Serialize> Request<P, R> {
    pub fn reply(&self, value: R) -> Answer {
        Answer(encode(value))
    }
}

impl<P, R> Deref for Request<P, R> {
    type Target = P;

    fn deref(&self) -> &P {
        &self.params
    }
}

impl<'de, P: Deserialize<'de>, R> Deserialize<'de> for Request<P, R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            params: P::deserialize(deserializer)?,
            reply: PhantomData,
        })
    }
}

impl<P: JsonSchema, R: JsonSchema> JsonSchema for Request<P, R> {
    fn schema_name() -> Cow<'static, str> {
        format!("Request_{}_{}", P::schema_name(), R::schema_name()).into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = P::json_schema(generator);
        // The variant's doc describes the request; the params struct's would override it.
        schema.remove("description");
        schema.insert(
            "x-returns".into(),
            generator.subschema_for::<R>().to_value(),
        );
        schema
    }
}

/// The reply to a query or action, built only by `reply`.
pub struct Answer(Value);

impl Answer {
    pub(crate) fn into_value(self) -> Value {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::decode;
    use schemars::schema_for;
    use serde_json::json;

    /// A spot.
    #[derive(Deserialize, JsonSchema)]
    struct At {
        x: u32,
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Queries {
        /// Doubles x.
        Double(Request<At, u32>),
        Name(Request<(), Option<String>>),
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum Actions {
        Wait(Request<(), ()>),
    }

    fn answer(query: Queries) -> Value {
        match query {
            Queries::Double(q) => q.reply(q.x * 2),
            Queries::Name(q) => q.reply(None),
        }
        .into_value()
    }

    #[test]
    fn decodes_params_and_answers() {
        assert_eq!(
            answer(decode(&json!({"type": "double", "x": 4})).unwrap()),
            json!(8)
        );
        assert_eq!(
            answer(decode(&json!({"type": "name"})).unwrap()),
            json!(null)
        );
        assert!(decode::<Queries>(&json!({"type": "double"})).is_err());
        let Actions::Wait(a) = decode(&json!({"type": "wait"})).unwrap();
        assert_eq!(a.reply(()).into_value(), json!(null));
    }

    #[test]
    fn variant_schema_carries_the_reply_type() {
        let schema = schema_for!(Queries).to_value();
        let [double, name] = schema["oneOf"].as_array().unwrap().as_slice() else {
            panic!("two variants: {schema}");
        };
        assert_eq!(double["description"], "Doubles x.");
        assert_eq!(double["properties"]["type"]["const"], "double");
        assert_eq!(double["properties"]["x"]["type"], "integer");
        assert_eq!(double["x-returns"]["type"], "integer");
        assert_eq!(name["description"], Value::Null);
        assert_eq!(name["properties"]["type"]["const"], "name");
        assert!(name["x-returns"]["type"].as_array().is_some());
        let schema = schema_for!(Actions).to_value();
        assert_eq!(schema["oneOf"][0]["x-returns"]["type"], "null");
    }
}
