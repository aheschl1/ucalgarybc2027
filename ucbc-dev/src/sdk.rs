//! Generates a game's Python API module from its [`GameApi`]: one class per type a
//! bot sees, one method per query and action. The hand-written handle subclasses
//! the generated `<Game>Api`.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::{Map, Value};
use ucbc_engine::GameApi;

type Schema = Map<String, Value>;

#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Int,
    Float,
    Str,
    Bool,
    Unit,
    List(Box<Ty>),
    Optional(Box<Ty>),
    Named(String),
}

struct Field {
    name: String,
    ty: Ty,
    doc: Option<String>,
}

enum Def {
    Enum(Vec<String>),
    Class(Vec<Field>),
    /// A tagged union: each `type` tag and the class it decodes to.
    Union(Vec<(String, String)>),
}

struct Named {
    doc: Option<String>,
    def: Def,
}

struct Method {
    name: String,
    doc: Option<String>,
    params: Vec<Field>,
    /// `_query` or `_act`.
    call: &'static str,
    returns: Ty,
}

/// Every type the module defines, by name.
#[derive(Default)]
struct Types(BTreeMap<String, Named>);

impl Types {
    fn add(&mut self, name: &str, doc: Option<String>, def: Def) -> Result<(), String> {
        let named = Named { doc, def };
        match self.0.get(name) {
            None => {
                self.0.insert(name.to_string(), named);
                Ok(())
            }
            Some(existing) if same(&existing.def, &named.def) => Ok(()),
            Some(_) => Err(format!("two different types are both named `{name}`")),
        }
    }

    /// Named types from a schema's `$defs`.
    fn collect(&mut self, root: &Schema) -> Result<(), String> {
        if let Some(defs) = root.get("$defs") {
            for (name, def) in obj(defs)? {
                self.define(name, obj(def)?)?;
            }
        }
        Ok(())
    }

    fn define(&mut self, name: &str, s: &Schema) -> Result<(), String> {
        let doc = description(s);
        let def = if let Some(values) = s.get("enum") {
            Def::Enum(strings(values)?)
        } else if let Some(variants) = variants(s)? {
            let mut tags = Vec::new();
            for (tag, fields, vdoc) in variants {
                let class = format!("{name}{}", pascal(&tag));
                self.add(&class, vdoc, Def::Class(fields))?;
                tags.push((tag, class));
            }
            Def::Union(tags)
        } else if s.get("type") == Some(&Value::String("object".into())) {
            Def::Class(fields(s)?)
        } else {
            return Err(format!(
                "`{name}`: only objects, string enums, and tagged enums can be types"
            ));
        };
        self.add(name, doc, def)
    }

    /// The type a response schema describes.
    fn response(&mut self, root: &Schema) -> Result<Ty, String> {
        self.collect(root)?;
        if root.get("type") == Some(&Value::String("null".into())) {
            return Ok(Ty::Unit);
        }
        let Some(Value::String(title)) = root.get("title") else {
            return Err("a response type needs a name; use a struct or enum".into());
        };
        self.define(title, root)?;
        Ok(Ty::Named(title.clone()))
    }

    /// Methods from a query or action schema: one per variant.
    fn methods(
        &mut self,
        root: &Schema,
        call: &'static str,
        returns: &Ty,
    ) -> Result<Vec<Method>, String> {
        self.collect(root)?;
        let variants = variants(root)?.ok_or("queries and actions must be tagged enums")?;
        Ok(variants
            .into_iter()
            .map(|(tag, params, doc)| Method {
                name: tag,
                doc,
                params,
                call,
                returns: returns.clone(),
            })
            .collect())
    }

    fn kind(&self, name: &str) -> &Def {
        &self.0.get(name).expect("named types are defined").def
    }

    /// Python that turns JSON `value` into `ty`.
    fn decode(&self, ty: &Ty, value: &str) -> String {
        match ty {
            Ty::Int | Ty::Float | Ty::Str | Ty::Bool => value.to_string(),
            Ty::Unit => "None".to_string(),
            Ty::List(inner) => match self.decode(inner, "v") {
                v if v == "v" => format!("list({value})"),
                each => format!("[{each} for v in {value}]"),
            },
            Ty::Optional(inner) => {
                format!("None if {value} is None else {}", self.decode(inner, value))
            }
            Ty::Named(name) => match self.kind(name) {
                Def::Enum(_) => format!("{name}({value})"),
                Def::Class(_) => format!("{name}._from({value})"),
                Def::Union(_) => format!("_from_{name}({value})"),
            },
        }
    }

    /// Python that turns `value` of `ty` into JSON.
    fn encode(&self, ty: &Ty, value: &str) -> String {
        match ty {
            Ty::Int | Ty::Float | Ty::Str | Ty::Bool | Ty::Unit => value.to_string(),
            Ty::List(inner) => match self.encode(inner, "v") {
                v if v == "v" => value.to_string(),
                each => format!("[{each} for v in {value}]"),
            },
            Ty::Optional(inner) => {
                format!("None if {value} is None else {}", self.encode(inner, value))
            }
            Ty::Named(name) => match self.kind(name) {
                Def::Enum(_) => value.to_string(),
                Def::Class(_) | Def::Union(_) => format!("asdict({value})"),
            },
        }
    }
}

fn same(a: &Def, b: &Def) -> bool {
    match (a, b) {
        (Def::Enum(x), Def::Enum(y)) => x == y,
        (Def::Union(x), Def::Union(y)) => x == y,
        (Def::Class(x), Def::Class(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|(f, g)| f.name == g.name && f.ty == g.ty)
        }
        _ => false,
    }
}

fn obj(v: &Value) -> Result<&Schema, String> {
    v.as_object()
        .ok_or_else(|| format!("expected a schema object, got {v}"))
}

fn description(s: &Schema) -> Option<String> {
    s.get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn strings(v: &Value) -> Result<Vec<String>, String> {
    v.as_array()
        .ok_or("expected a list")?
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("enum value {x} is not a string"))
        })
        .collect()
}

/// The variants of a `#[serde(tag = "type")]` enum: tag, fields, doc.
#[allow(clippy::type_complexity)]
fn variants(s: &Schema) -> Result<Option<Vec<(String, Vec<Field>, Option<String>)>>, String> {
    let Some(one_of) = s.get("oneOf") else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for v in one_of.as_array().ok_or("oneOf must be a list")? {
        let mut v = obj(v)?.clone();
        let props = v.get_mut("properties").and_then(Value::as_object_mut);
        let tag = props
            .and_then(|p| p.remove("type"))
            .and_then(|t| t.get("const").and_then(Value::as_str).map(str::to_string))
            .ok_or("variant without a `type` tag")?;
        out.push((tag, fields(&v)?, description(&v)));
    }
    Ok(Some(out))
}

/// An object's fields, required ones first in declaration order.
fn fields(s: &Schema) -> Result<Vec<Field>, String> {
    let props = match s.get("properties") {
        Some(p) => obj(p)?,
        None => return Ok(Vec::new()),
    };
    let required = s
        .get("required")
        .map(strings)
        .transpose()?
        .unwrap_or_default();
    let mut names: Vec<&str> = required
        .iter()
        .map(String::as_str)
        .filter(|k| props.contains_key(*k))
        .collect();
    names.extend(
        props
            .keys()
            .map(String::as_str)
            .filter(|k| !required.iter().any(|r| r == k)),
    );
    names
        .into_iter()
        .map(|name| {
            let s = obj(&props[name])?;
            let ty = ty(s)?;
            let ty = if required.iter().any(|r| r == name) || matches!(ty, Ty::Optional(_)) {
                ty
            } else {
                Ty::Optional(Box::new(ty))
            };
            Ok(Field {
                name: name.to_string(),
                ty,
                doc: description(s),
            })
        })
        .collect()
}

fn ty(s: &Schema) -> Result<Ty, String> {
    if let Some(r) = s.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        return Ok(Ty::Named(name.to_string()));
    }
    if let Some(any_of) = s.get("anyOf").and_then(Value::as_array)
        && let [a, b] = any_of.as_slice()
    {
        let null = Value::String("null".into());
        let inner = match (obj(a)?.get("type"), obj(b)?.get("type")) {
            (Some(t), _) if *t == null => obj(b)?,
            (_, Some(t)) if *t == null => obj(a)?,
            _ => return Err(format!("unsupported anyOf: {}", Value::Object(s.clone()))),
        };
        return Ok(Ty::Optional(Box::new(ty(inner)?)));
    }
    let kind = match s.get("type") {
        Some(Value::String(k)) => k.as_str(),
        Some(Value::Array(kinds)) => {
            let mut base = s.clone();
            let others: Vec<&Value> = kinds
                .iter()
                .filter(|k| k.as_str() != Some("null"))
                .collect();
            if others.len() == kinds.len() || others.len() != 1 {
                return Err(format!(
                    "unsupported type list: {}",
                    Value::Array(kinds.clone())
                ));
            }
            base.insert("type".into(), others[0].clone());
            return Ok(Ty::Optional(Box::new(ty(&base)?)));
        }
        _ => {
            return Err(format!(
                "schema without a type: {}",
                Value::Object(s.clone())
            ));
        }
    };
    Ok(match kind {
        "integer" => Ty::Int,
        "number" => Ty::Float,
        "string" if s.get("enum").is_none() => Ty::Str,
        "boolean" => Ty::Bool,
        "null" => Ty::Unit,
        "array" => Ty::List(Box::new(ty(obj(s
            .get("items")
            .ok_or("array without items")?)?)?)),
        "object" => return Err("inline objects are not supported; use a named struct".into()),
        "string" => return Err("inline string enums are not supported; use a named enum".into()),
        other => return Err(format!("unsupported type `{other}`")),
    })
}

fn pascal(snake: &str) -> String {
    snake
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn member(value: &str) -> String {
    let name: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

fn py_type(ty: &Ty) -> String {
    match ty {
        Ty::Int => "int".into(),
        Ty::Float => "float".into(),
        Ty::Str => "str".into(),
        Ty::Bool => "bool".into(),
        Ty::Unit => "None".into(),
        Ty::List(inner) => format!("list[{}]", py_type(inner)),
        Ty::Optional(inner) => format!("{} | None", py_type(inner)),
        Ty::Named(name) => name.clone(),
    }
}

fn docstring(out: &mut String, indent: &str, doc: &str) {
    let doc = doc.trim().replace('\n', &format!("\n{indent}"));
    let _ = writeln!(out, "{indent}\"\"\"{doc}\"\"\"");
}

/// The Python module for `api`.
pub fn generate(api: &GameApi) -> Result<String, String> {
    let mut types = Types::default();
    let query_response = types.response(obj(&api.query_response)?)?;
    let action_response = types.response(obj(&api.action_response)?)?;
    let mut methods = types.methods(obj(&api.query)?, "_query", &query_response)?;
    methods.extend(types.methods(obj(&api.action)?, "_act", &action_response)?);

    let uses_asdict = methods
        .iter()
        .flat_map(|m| &m.params)
        .any(|p| types.encode(&p.ty, &p.name).contains("asdict"));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "\"\"\"Generated by `ucbc-dev gen-sdk` from the Rust types of the `{}` game. Do not edit.\"\"\"\n",
        api.name
    );
    out.push_str("from __future__ import annotations\n\n");
    out.push_str(if uses_asdict {
        "from dataclasses import asdict, dataclass\n"
    } else {
        "from dataclasses import dataclass\n"
    });
    if types.0.values().any(|t| matches!(t.def, Def::Enum(_))) {
        out.push_str("from enum import Enum\n");
    }
    out.push_str("from typing import Any, Self\n\nfrom ucbc.handle import Handle\n");

    // Unions last: their aliases name the variant classes when the module loads.
    let (unions, others): (Vec<_>, Vec<_>) = types
        .0
        .iter()
        .partition(|(_, t)| matches!(t.def, Def::Union(_)));
    for (name, named) in others.into_iter().chain(unions) {
        out.push_str("\n\n");
        match &named.def {
            Def::Enum(values) => {
                let _ = writeln!(out, "class {name}(str, Enum):");
                if let Some(doc) = &named.doc {
                    docstring(&mut out, "    ", doc);
                    out.push('\n');
                }
                for v in values {
                    let _ = writeln!(out, "    {} = \"{v}\"", member(v));
                }
            }
            Def::Class(fields) => {
                let _ = writeln!(out, "@dataclass(frozen=True)\nclass {name}:");
                if let Some(doc) = &named.doc {
                    docstring(&mut out, "    ", doc);
                    out.push('\n');
                }
                for f in fields {
                    let _ = writeln!(out, "    {}: {}", f.name, py_type(&f.ty));
                    if let Some(doc) = &f.doc {
                        docstring(&mut out, "    ", doc);
                    }
                }
                out.push_str(
                    "\n    @classmethod\n    def _from(cls, d: dict[str, Any]) -> Self:\n",
                );
                if fields.is_empty() {
                    out.push_str("        return cls()\n");
                } else {
                    out.push_str("        return cls(\n");
                    for f in fields {
                        let value = if matches!(f.ty, Ty::Optional(_)) {
                            format!("d.get(\"{}\")", f.name)
                        } else {
                            format!("d[\"{}\"]", f.name)
                        };
                        let _ = writeln!(
                            out,
                            "            {}={},",
                            f.name,
                            types.decode(&f.ty, &value)
                        );
                    }
                    out.push_str("        )\n");
                }
            }
            Def::Union(tags) => {
                let classes: Vec<&str> = tags.iter().map(|(_, c)| c.as_str()).collect();
                let _ = writeln!(out, "{name} = {}", classes.join(" | "));
                if let Some(doc) = &named.doc {
                    docstring(&mut out, "", doc);
                }
                let _ = writeln!(out, "\n\ndef _from_{name}(d: dict[str, Any]) -> {name}:");
                out.push_str("    match d[\"type\"]:\n");
                for (tag, class) in tags {
                    let _ = writeln!(
                        out,
                        "        case \"{tag}\":\n            return {class}._from(d)"
                    );
                }
                let _ = writeln!(
                    out,
                    "    raise ValueError(f\"unknown {name} type {{d['type']!r}}\")"
                );
            }
        }
    }

    let _ = write!(
        out,
        "\n\nclass {}Api(Handle):\n    \"\"\"Queries and actions of the `{}` game, one method each.\"\"\"\n",
        api.type_name, api.name
    );
    for m in &methods {
        let params: Vec<String> = m
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, py_type(&p.ty)))
            .collect();
        let _ = write!(out, "\n    def {}(self", m.name);
        for p in &params {
            let _ = write!(out, ", {p}");
        }
        let _ = writeln!(out, ") -> {}:", py_type(&m.returns));
        if let Some(doc) = &m.doc {
            docstring(&mut out, "        ", doc);
        }
        let mut payload = format!("{{\"type\": \"{}\"", m.name);
        for p in &m.params {
            let _ = write!(
                payload,
                ", \"{}\": {}",
                p.name,
                types.encode(&p.ty, &p.name)
            );
        }
        payload.push('}');
        let call = format!("self.{}({payload})", m.call);
        if m.returns == Ty::Unit {
            let _ = writeln!(out, "        {call}");
        } else {
            let _ = writeln!(out, "        return {}", types.decode(&m.returns, &call));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn api() -> GameApi {
        GameApi {
            name: "demo",
            type_name: "Demo",
            query: json!({
                "oneOf": [
                    {"type": "object", "required": ["type"], "properties": {"type": {"const": "look"}},
                     "description": "Look around."},
                    {"type": "object", "required": ["type", "at"],
                     "properties": {"type": {"const": "peek"}, "at": {"$ref": "#/$defs/Spot"}}},
                ],
                "$defs": {"Spot": {"type": "object", "required": ["x", "y"],
                                    "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}}}},
            }),
            query_response: json!({
                "title": "Seen", "type": "object", "required": ["kind"],
                "properties": {"spots": {"type": "array", "items": {"$ref": "#/$defs/Spot"}},
                               "kind": {"$ref": "#/$defs/Kind"}},
                "$defs": {"Kind": {"type": "string", "enum": ["near", "far"]},
                          "Spot": {"type": "object", "required": ["x", "y"],
                                    "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}}}},
            }),
            action: json!({
                "oneOf": [{"type": "object", "required": ["type", "how"],
                           "properties": {"type": {"const": "go"}, "how": {"$ref": "#/$defs/Kind"}}}],
                "$defs": {"Kind": {"type": "string", "enum": ["near", "far"]}},
            }),
            action_response: json!({
                "title": "Went",
                "oneOf": [
                    {"type": "object", "required": ["type"], "properties": {"type": {"const": "ok"}}},
                    {"type": "object", "required": ["type", "why"],
                     "properties": {"type": {"const": "blocked"}, "why": {"type": "string"}}},
                ],
            }),
            snapshot: json!({"type": "null"}),
        }
    }

    #[test]
    fn generates_types_and_methods() {
        let py = generate(&api()).unwrap();
        for line in [
            "class Kind(str, Enum):",
            "    NEAR = \"near\"",
            "class Seen:",
            "    spots: list[Spot] | None",
            "            spots=None if d.get(\"spots\") is None else [Spot._from(v) for v in d.get(\"spots\")],",
            "Went = WentOk | WentBlocked",
            "def _from_Went(d: dict[str, Any]) -> Went:",
            "        case \"blocked\":\n            return WentBlocked._from(d)",
            "    raise ValueError(f\"unknown Went type {d['type']!r}\")",
            "class DemoApi(Handle):",
            "    def look(self) -> Seen:",
            "        \"\"\"Look around.\"\"\"",
            "        return Seen._from(self._query({\"type\": \"look\"}))",
            "    def peek(self, at: Spot) -> Seen:",
            "        return Seen._from(self._query({\"type\": \"peek\", \"at\": asdict(at)}))",
            "    def go(self, how: Kind) -> Went:",
            "        return _from_Went(self._act({\"type\": \"go\", \"how\": how}))",
            "from dataclasses import asdict, dataclass",
        ] {
            assert!(py.contains(line), "missing {line:?} in:\n{py}");
        }
        let at = |s: &str| py.find(s).unwrap();
        assert!(
            at("Went = ") > at("class WentOk:"),
            "union before its classes:\n{py}"
        );
    }

    #[test]
    fn refuses_what_it_cannot_express() {
        let mut bad = api();
        bad.query_response = json!({"title": "Odd", "type": "object",
            "properties": {"inline": {"type": "object", "properties": {}}}});
        assert!(generate(&bad).unwrap_err().contains("inline objects"));
    }
}
