//! The content files, loaded once. Ids are generated enums (build.rs); everything else
//! about an item or building is data here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

include!(concat!(env!("OUT_DIR"), "/ids.rs"));

impl Item {
    pub fn def(self) -> &'static ItemDef {
        &content().items[&self]
    }
}

impl Building {
    pub fn def(self) -> &'static BuildingDef {
        &content().buildings[&self]
    }
}

impl Crop {
    pub fn def(self) -> &'static CropDef {
        &content().crops[&self]
    }

    /// The crop `item` plants, if it is a seed.
    pub fn from_seed(item: Item) -> Option<Crop> {
        content()
            .crops
            .iter()
            .find(|(_, c)| c.seed == item)
            .map(|(&crop, _)| crop)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    pub food: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropDef {
    pub seed: Item,
    pub grows: u32,
    #[serde(rename = "yield")]
    pub gives: BTreeMap<Item, u32>,
    pub tires: u32,
    pub regrows: Option<u32>,
}

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
    /// Range it waters farmland in.
    pub water: Option<usize>,
}

pub struct Content {
    pub items: BTreeMap<Item, ItemDef>,
    pub buildings: BTreeMap<Building, BuildingDef>,
    pub crops: BTreeMap<Crop, CropDef>,
}

pub fn content() -> &'static Content {
    static CONTENT: OnceLock<Content> = OnceLock::new();
    CONTENT.get_or_init(|| Content {
        items: toml::from_str(include_str!("../content/items.toml")).expect("items.toml"),
        buildings: toml::from_str(include_str!("../content/buildings.toml"))
            .expect("buildings.toml"),
        crops: toml::from_str(include_str!("../content/crops.toml")).expect("crops.toml"),
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

    /// A seed plants one crop, and every crop gives something.
    #[test]
    fn crops_have_their_own_seeds_and_a_yield() {
        let crops = &content().crops;
        for (&crop, def) in crops {
            assert_eq!(Crop::from_seed(def.seed), Some(crop));
            assert!(!def.gives.is_empty(), "{crop:?}");
            assert!(def.seed.def().food.is_none(), "{crop:?}'s seed is food");
        }
    }
}
