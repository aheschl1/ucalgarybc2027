//! Generates the `Item`, `Building`, and `Crop` enums from the content files' keys, so ids are
//! types in Rust and in the generated SDK while their details stay data. Variants are in
//! alphabetical order, whatever the file's: observations send a variant as its position.

use std::fmt::Write;
use std::{env, fs};

fn variants(file: &str) -> Vec<String> {
    println!("cargo::rerun-if-changed={file}");
    let text = fs::read_to_string(file).unwrap();
    let table: toml::Table = text.parse().unwrap_or_else(|e| panic!("{file}: {e}"));
    table.keys().cloned().collect()
}

fn pascal(id: &str) -> String {
    id.split('_')
        .map(|w| w[..1].to_uppercase() + &w[1..])
        .collect()
}

fn enum_of(out: &mut String, name: &str, ids: &[String]) {
    let derives = "Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, \
                   serde::Serialize, serde::Deserialize, schemars::JsonSchema";
    writeln!(
        out,
        "#[derive({derives})]\n#[serde(rename_all = \"snake_case\")]"
    )
    .unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();
    for id in ids {
        writeln!(out, "    {},", pascal(id)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

fn main() {
    let mut out = String::new();
    enum_of(&mut out, "Item", &variants("content/items.toml"));
    enum_of(&mut out, "Building", &variants("content/buildings.toml"));
    enum_of(&mut out, "Crop", &variants("content/crops.toml"));
    fs::write(format!("{}/ids.rs", env::var("OUT_DIR").unwrap()), out).unwrap();
}
