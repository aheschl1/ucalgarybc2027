//! The content files, loaded once. Ids are generated enums (build.rs); everything else
//! about an item or building is data here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

include!(concat!(env!("OUT_DIR"), "/ids.rs"));

impl Building {
    pub fn def(self) -> &'static BuildingDef {
        &content().buildings[&self]
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingDef {
    pub hp: u32,
    pub cost: BTreeMap<Item, u32>,
    /// `build` actions to finish it once placed.
    pub work: u32,
    pub upgrades: Option<Building>,
    /// Items it holds, of any kind together.
    pub storage: Option<u32>,
    /// Range of the team's radio network around it.
    pub radio: Option<usize>,
}

pub struct Content {
    pub items: BTreeMap<Item, ItemDef>,
    pub buildings: BTreeMap<Building, BuildingDef>,
}

pub fn content() -> &'static Content {
    static CONTENT: OnceLock<Content> = OnceLock::new();
    CONTENT.get_or_init(|| Content {
        items: toml::from_str(include_str!("../content/items.toml")).expect("items.toml"),
        buildings: toml::from_str(include_str!("../content/buildings.toml"))
            .expect("buildings.toml"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each `upgrades` line ends at a building that upgrades nothing, without a cycle.
    #[test]
    fn upgrade_lines_end() {
        let buildings = &content().buildings;
        for &start in buildings.keys() {
            let mut at = start;
            for _ in 0..=buildings.len() {
                match at.def().upgrades {
                    Some(prev) => at = prev,
                    None => break,
                }
            }
            assert!(
                at.def().upgrades.is_none(),
                "{start:?} is on an upgrade cycle"
            );
        }
    }
}
