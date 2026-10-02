//! Generates a game's Python API module from its [`GameApi`]: one class per type a
//! bot sees, one method per query and action. The hand-written handle subclasses
//! the generated `<Game>Api`.

use std::collections::{BTreeMap, BTreeSet};
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

/// One variant of a `#[serde(tag = "type")]` enum.
struct Variant {
    tag: String,
    fields: Vec<Field>,
    doc: Option<String>,
    /// The variant's reply type, from its `x-returns`.
    returns: Option<Ty>,
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
            for v in variants {
                let class = format!("{name}{}", pascal(&v.tag));
                self.add(&class, v.doc, Def::Class(v.fields))?;
                tags.push((v.tag, class));
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

    /// Methods from a query or action schema, one per variant, each returning its
    /// variant's reply type.
    fn methods(&mut self, root: &Schema, call: &'static str) -> Result<Vec<Method>, String> {
        self.collect(root)?;
        let variants = variants(root)?.ok_or("queries and actions must be tagged enums")?;
        variants
            .into_iter()
            .map(|v| {
                let returns = v
                    .returns
                    .ok_or_else(|| format!("`{}` must hold a `Request<P, R>`", v.tag))?;
                Ok(Method {
                    name: v.tag,
                    doc: v.doc,
                    params: v.fields,
                    call,
                    returns,
                })
            })
            .collect()
    }

    fn kind(&self, name: &str) -> &Def {
        &self.0.get(name).expect("named types are defined").def
    }

    /// The `type` tag of a union's variant class.
    fn tag_of(&self, class: &str) -> Option<&str> {
        self.0.values().find_map(|t| match &t.def {
            Def::Union(tags) => tags
                .iter()
                .find(|(_, c)| c == class)
                .map(|(tag, _)| tag.as_str()),
            _ => None,
        })
    }

    /// Classes a method argument can hold, at any depth: the ones that need `_to`.
    fn encoded(&self, methods: &[Method]) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut todo: Vec<Ty> = methods
            .iter()
            .flat_map(|m| m.params.iter().map(|p| p.ty.clone()))
            .collect();
        while let Some(ty) = todo.pop() {
            match ty {
                Ty::List(inner) | Ty::Optional(inner) => todo.push(*inner),
                Ty::Named(name) => match self.kind(&name) {
                    Def::Class(fields) if out.insert(name.clone()) => {
                        todo.extend(fields.iter().map(|f| f.ty.clone()));
                    }
                    Def::Union(tags) => {
                        todo.extend(tags.iter().map(|(_, c)| Ty::Named(c.clone())));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        out
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
            Ty::Optional(inner) => match self.decode(inner, value) {
                v if v == value => v,
                each => format!("None if {value} is None else {each}"),
            },
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
                Def::Class(_) | Def::Union(_) => format!("{value}._to()"),
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

/// The variants of a `#[serde(tag = "type")]` enum.
fn variants(s: &Schema) -> Result<Option<Vec<Variant>>, String> {
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
        out.push(Variant {
            tag,
            fields: fields(&v)?,
            doc: description(&v),
            returns: v
                .get("x-returns")
                .map(|r| obj(r).and_then(ty))
                .transpose()?,
        });
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
    let mut methods = types.methods(obj(&api.query)?, "_query")?;
    methods.extend(types.methods(obj(&api.action)?, "_act")?);

    let encoded = types.encoded(&methods);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "\"\"\"Generated by `ucbc-dev gen-sdk` from the Rust types of the `{}` game. Do not edit.\"\"\"\n",
        api.name
    );
    // Only the imports something uses: a game may expose no classes at all.
    let has = |kind: fn(&Def) -> bool| types.0.values().any(|t| kind(&t.def));
    let classes = has(|d| matches!(d, Def::Class(_)));
    let unions = has(|d| matches!(d, Def::Union(_)));
    let enums = has(|d| matches!(d, Def::Enum(_)));
    out.push_str("from __future__ import annotations\n\n");
    if classes {
        out.push_str("from dataclasses import dataclass\n");
    }
    if enums {
        out.push_str("from enum import Enum\n");
    }
    if classes {
        out.push_str("from typing import Any, Self\n");
    } else if unions {
        out.push_str("from typing import Any\n");
    }
    if classes || unions || enums {
        out.push('\n');
    }
    out.push_str("from ucbc.handle import Handle\n");

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
                if encoded.contains(name) {
                    out.push_str("\n    def _to(self) -> dict[str, Any]:\n");
                    let mut entries: Vec<String> = types
                        .tag_of(name)
                        .map(|tag| format!("\"type\": \"{tag}\""))
                        .into_iter()
                        .collect();
                    for f in fields {
                        let value = types.encode(&f.ty, &format!("self.{}", f.name));
                        entries.push(format!("\"{}\": {value}", f.name));
                    }
                    let _ = writeln!(out, "        return {{{}}}", entries.join(", "));
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
        let decoded = types.decode(&m.returns, "reply");
        if m.returns == Ty::Unit {
            let _ = writeln!(out, "        {call}");
        } else if decoded == "reply" {
            // Already the Python value; annotated, so the method does not return `Any`.
            let ty = py_type(&m.returns);
            let _ = writeln!(out, "        reply: {ty} = {call}\n        return reply");
        } else if matches!(m.returns, Ty::Optional(_)) {
            // The decode reads the reply twice.
            let _ = writeln!(out, "        reply = {call}\n        return {decoded}");
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
                     "description": "Look around.", "x-returns": {"$ref": "#/$defs/Seen"}},
                    {"type": "object", "required": ["type", "at"],
                     "properties": {"type": {"const": "peek"}, "at": {"$ref": "#/$defs/Spot"}},
                     "x-returns": {"anyOf": [{"$ref": "#/$defs/Kind"}, {"type": "null"}]}},
                    {"type": "object", "required": ["type"], "properties": {"type": {"const": "count"}},
                     "x-returns": {"type": "integer"}},
                ],
                "$defs": {"Seen": {"type": "object", "required": ["kind"],
                                   "properties": {"spots": {"type": "array", "items": {"$ref": "#/$defs/Spot"}},
                                                  "kind": {"$ref": "#/$defs/Kind"}}},
                          "Kind": {"type": "string", "enum": ["near", "far"]},
                          "Spot": {"type": "object", "required": ["x", "y"],
                                    "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}}}},
            }),
            action: json!({
                "oneOf": [{"type": "object", "required": ["type", "how"],
                           "properties": {"type": {"const": "go"}, "how": {"$ref": "#/$defs/Kind"}},
                           "x-returns": {"$ref": "#/$defs/Went"}}],
                "$defs": {"Kind": {"type": "string", "enum": ["near", "far"]},
                          "Went": {"oneOf": [
                              {"type": "object", "required": ["type"], "properties": {"type": {"const": "ok"}}},
                              {"type": "object", "required": ["type", "why"],
                               "properties": {"type": {"const": "blocked"}, "why": {"type": "string"}}},
                          ]}},
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
            "    def peek(self, at: Spot) -> Kind | None:",
            "        reply = self._query({\"type\": \"peek\", \"at\": at._to()})\n        return None if reply is None else Kind(reply)",
            "    def _to(self) -> dict[str, Any]:\n        return {\"x\": self.x, \"y\": self.y}",
            "    def count(self) -> int:",
            "        reply: int = self._query({\"type\": \"count\"})\n        return reply",
            "    def go(self, how: Kind) -> Went:",
            "        return _from_Went(self._act({\"type\": \"go\", \"how\": how}))",
        ] {
            assert!(py.contains(line), "missing {line:?} in:\n{py}");
        }
        let at = |s: &str| py.find(s).unwrap();
        assert!(
            at("Went = ") > at("class WentOk:"),
            "union before its classes:\n{py}"
        );
        // Only argument types are encoded.
        assert_eq!(py.matches("def _to(").count(), 1, "{py}");
    }

    #[test]
    fn union_arguments_carry_their_tag() {
        let mut api = api();
        api.action["oneOf"][0]["properties"]["how"] = json!({"$ref": "#/$defs/Went"});
        let py = generate(&api).unwrap();
        for line in [
            "    def go(self, how: Went) -> Went:",
            "        return _from_Went(self._act({\"type\": \"go\", \"how\": how._to()}))",
            "    def _to(self) -> dict[str, Any]:\n        return {\"type\": \"ok\"}",
            "    def _to(self) -> dict[str, Any]:\n        return {\"type\": \"blocked\", \"why\": self.why}",
        ] {
            assert!(py.contains(line), "missing {line:?} in:\n{py}");
        }
    }

    #[test]
    fn imports_only_what_it_uses() {
        let mut api = api();
        api.query = json!({
            "oneOf": [{"type": "object", "required": ["type"], "properties": {"type": {"const": "kind"}},
                       "x-returns": {"$ref": "#/$defs/Kind"}}],
            "$defs": {"Kind": {"type": "string", "enum": ["near", "far"]}},
        });
        api.action = json!({"oneOf": [{"type": "object", "required": ["type"],
                                       "properties": {"type": {"const": "wait"}},
                                       "x-returns": {"type": "null"}}]});
        let py = generate(&api).unwrap();
        assert!(
            py.contains("from __future__ import annotations\n\nfrom enum import Enum\n\nfrom ucbc.handle import Handle\n"),
            "{py}"
        );
        assert!(!py.contains("dataclass") && !py.contains("typing"), "{py}");
    }

    #[test]
    fn refuses_what_it_cannot_express() {
        let mut bad = api();
        bad.query["oneOf"][2]["x-returns"] = json!({"type": "object", "properties": {}});
        assert!(generate(&bad).unwrap_err().contains("inline objects"));
        bad.query["oneOf"][2]
            .as_object_mut()
            .unwrap()
            .remove("x-returns");
        assert!(
            generate(&bad)
                .unwrap_err()
                .contains("`count` must hold a `Request<P, R>`")
        );
        let mut bad = api();
        bad.action["oneOf"][0]
            .as_object_mut()
            .unwrap()
            .remove("x-returns");
        assert!(
            generate(&bad)
                .unwrap_err()
                .contains("`go` must hold a `Request<P, R>`")
        );
    }
}
