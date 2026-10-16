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

impl Recipe {
    pub fn def(self) -> &'static RecipeDef {
        &content().recipes[&self]
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
    pub tool: Option<u32>,
    pub weapon: Option<u32>,
    pub armour: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeDef {
    pub at: Building,
    #[serde(rename = "in")]
    pub inputs: BTreeMap<Item, u32>,
    #[serde(rename = "out")]
    pub outputs: BTreeMap<Item, u32>,
    pub ticks: u32,
    #[serde(default)]
    pub known: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrillDef {
    /// Items one `operate` takes from the tile under it.
    pub rate: u32,
    /// The highest vein tier it reaches.
    pub tier: u32,
    /// Burned from its storage, one per `operate`.
    pub burns: Option<Item>,
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
    /// Units `study` items at it.
    #[serde(default)]
    pub research: bool,
    pub drill: Option<DrillDef>,
    /// Its team walks through it.
    #[serde(default)]
    pub gate: bool,
    /// Units next to it attack within this range of it, and take half damage.
    pub tower: Option<usize>,
    /// Finishing it wins the set.
    #[serde(default)]
    pub monument: bool,
    #[serde(default)]
    pub known: bool,
}

pub struct Content {
    pub items: BTreeMap<Item, ItemDef>,
    pub buildings: BTreeMap<Building, BuildingDef>,
    pub crops: BTreeMap<Crop, CropDef>,
    pub recipes: BTreeMap<Recipe, RecipeDef>,
}

pub fn content() -> &'static Content {
    static CONTENT: OnceLock<Content> = OnceLock::new();
    CONTENT.get_or_init(|| Content {
        items: toml::from_str(include_str!("../content/items.toml")).expect("items.toml"),
        buildings: toml::from_str(include_str!("../content/buildings.toml"))
            .expect("buildings.toml"),
        crops: toml::from_str(include_str!("../content/crops.toml")).expect("crops.toml"),
        recipes: toml::from_str(include_str!("../content/recipes.toml")).expect("recipes.toml"),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::industry::Known;
    use crate::map::RESOURCES;
    use crate::rules::{HAND_TIER, START};

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

    /// A team starting with nothing but its base reaches every item, recipe, and
    /// building: it gathers what its best tool or a drill it can build reaches, grows its
    /// seeds, crafts the recipes it knows at buildings it can build, and, once it can
    /// build a research building, studies everything it has.
    #[test]
    fn a_team_reaches_everything() {
        let c = content();
        let mut known = Known::start();
        let mut had: BTreeSet<Item> = START.map(|(item, _)| item).into_iter().collect();
        loop {
            let before = (had.len(), known.recipes.len(), known.buildings.len());
            let buildable = |b: &Building| b.def().cost.keys().all(|i| had.contains(i));
            let usable: Vec<&BuildingDef> = known
                .buildings
                .iter()
                .filter(|b| buildable(b))
                .map(|b| b.def())
                .collect();
            let tools = had.iter().filter_map(|i| i.def().tool);
            let drills = usable
                .iter()
                .filter_map(|b| b.drill.as_ref().map(|d| d.tier));
            let reach = tools.chain(drills).fold(HAND_TIER, u32::max);
            let mut new: Vec<Item> = RESOURCES
                .iter()
                .filter(|r| r.3 <= reach)
                .map(|r| r.1)
                .collect();
            new.extend(
                c.crops
                    .values()
                    .filter(|crop| had.contains(&crop.seed))
                    .flat_map(|crop| crop.gives.keys()),
            );
            for recipe in &known.recipes {
                let r = recipe.def();
                if known.buildings.contains(&r.at)
                    && buildable(&r.at)
                    && r.inputs.keys().all(|i| had.contains(i))
                {
                    new.extend(r.outputs.keys());
                }
            }
            let research = usable.iter().any(|b| b.research);
            had.extend(new);
            if research {
                for &item in &had {
                    known.study(item);
                }
            }
            if before == (had.len(), known.recipes.len(), known.buildings.len()) {
                break;
            }
        }
        let missing = |all: Vec<String>| all.is_empty();
        let items: Vec<String> = c
            .items
            .keys()
            .filter(|i| !had.contains(i))
            .map(|i| format!("{i:?}"))
            .collect();
        let recipes: Vec<String> = c
            .recipes
            .keys()
            .filter(|r| !known.recipes.contains(r))
            .map(|r| format!("{r:?}"))
            .collect();
        let buildings: Vec<String> = c
            .buildings
            .keys()
            .filter(|b| !known.buildings.contains(b))
            .map(|b| format!("{b:?}"))
            .collect();
        assert!(
            missing(items.clone()) && missing(recipes.clone()) && missing(buildings.clone()),
            "never reached: {items:?} {recipes:?} {buildings:?}"
        );
    }

    /// Recipes are made where things are stored, and drills that burn fuel can hold it.
    #[test]
    fn behaviours_have_what_they_need() {
        let c = content();
        for (recipe, def) in &c.recipes {
            assert!(
                def.at.def().storage.is_some(),
                "{recipe:?} is made where nothing is stored"
            );
        }
        for (building, def) in &c.buildings {
            if def.drill.as_ref().is_some_and(|d| d.burns.is_some()) {
                assert!(
                    def.storage.is_some(),
                    "{building:?} burns fuel it can't hold"
                );
            }
        }
    }
}
