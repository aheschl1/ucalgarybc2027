//! A query names its reply type, so a game can only answer it with that type and the
//! generated Python API can type each query method.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::Deref;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::payload::encode;

/// One variant of a game's query enum: parameters `Q`, answered with an `R`.
/// Derefs to `Q`. Its schema is `Q`'s with `R`'s under `x-returns`.
pub struct Query<Q, R> {
    params: Q,
    reply: PhantomData<fn(R)>,
}

impl<Q, R: Serialize> Query<Q, R> {
    pub fn reply(&self, value: R) -> Answer {
        Answer(encode(value))
    }
}

impl<Q, R> Deref for Query<Q, R> {
    type Target = Q;

    fn deref(&self) -> &Q {
        &self.params
    }
}

impl<'de, Q: Deserialize<'de>, R> Deserialize<'de> for Query<Q, R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            params: Q::deserialize(deserializer)?,
            reply: PhantomData,
        })
    }
}

impl<Q: JsonSchema, R: JsonSchema> JsonSchema for Query<Q, R> {
    fn schema_name() -> Cow<'static, str> {
        format!("Query_{}_{}", Q::schema_name(), R::schema_name()).into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = Q::json_schema(generator);
        // The variant's doc describes the query; the params struct's would override it.
        schema.remove("description");
        schema.insert(
            "x-returns".into(),
            generator.subschema_for::<R>().to_value(),
        );
        schema
    }
}

/// A query's reply, built only by [`Query::reply`].
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
        Double(Query<At, u32>),
        Name(Query<(), Option<String>>),
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
    }
}
