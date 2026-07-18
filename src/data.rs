//! Embedded game data: config, content registries, and shared delta types.

pub mod contracts;
pub mod crew;
pub mod events;
pub mod legacies;
pub mod ship_components;

use macroquad_toolkit::assets::TextureConfig;
use macroquad_toolkit::data_loader::{
    load_embedded_json, load_embedded_json_labeled, DataRegistry,
};
use serde::{Deserialize, Serialize};

use contracts::ContractTemplate;
use crew::{CrewArchetype, DynastyNamePools};
use events::EventTemplate;
use legacies::LegacyDef;
use ship_components::ShipComponentCatalog;

const GAME_CONFIG_JSON: &str = include_str!("../assets/data/game_config.json");
const TEXTURE_MANIFEST_JSON: &str = include_str!("../assets/data/texture_manifest.json");
const SHIP_COMPONENTS_JSON: &str = include_str!("../assets/ship_components.json");
const EVENTS_JSON: &str = include_str!("../assets/events.json");
const LEGACIES_JSON: &str = include_str!("../assets/legacies.json");
const CONTRACTS_JSON: &str = include_str!("../assets/contracts.json");
const DYNASTY_NAMES_JSON: &str = include_str!("../assets/dynasty_names.json");
const CREW_ARCHETYPES_JSON: &str = include_str!("../assets/crew_archetypes.json");

/// Signed per-resource change used by event outcomes, costs, and rewards.
/// Also doubles as an absolute amount set (e.g. starting resources).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceDelta {
    pub credits: i64,
    pub energy: i64,
    pub minerals: i64,
    pub food: i64,
    pub influence: i64,
}

/// Per-year production rates for each tracked resource. Initialized with every
/// key present so colonization/component bonuses always have a slot to land in
/// (the original web build lost these bonuses to a missing-key bug — GDD §5.1).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProductionRates {
    pub credits: f32,
    pub energy: f32,
    pub minerals: f32,
    pub food: f32,
    pub influence: f32,
}

/// Signed change to ship-condition stats.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ShipDelta {
    pub hull_integrity: f32,
    pub life_support: f32,
    pub fuel: f32,
    pub spare_parts: i32,
}

/// Signed change to colony-scale population stats.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PopulationDelta {
    pub count: i32,
    pub morale: f32,
    pub unity: f32,
    pub stability: f32,
    pub legacy_loyalty: f32,
    pub adaptation: f32,
    pub cultural_drift: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub game_name: String,
    pub display_name: String,
    pub save_slot: String,
    pub chronicle_slot: String,
    pub version: String,
    pub starting_resources: ResourceDelta,
    pub base_production: ProductionRates,
    pub starting_population: u32,
    pub food_per_person_per_year: f32,
    pub low_food_threshold: i64,
    pub low_energy_threshold: i64,
    pub hull_warning_threshold: f32,
    pub life_support_warning_threshold: f32,
    pub hull_decay_per_year: f32,
    pub life_support_decay_per_year: f32,
    pub generation_interval_years: u32,
    pub leader_retirement_age: u32,
    pub heir_min_age: u32,
    pub heir_max_age: u32,
    pub member_max_age: u32,
    pub event_chance_base: f32,
    pub event_chance_cap: f32,
    /// Chance a legacy dilemma confronts each new generation (GDD §5.5).
    pub dilemma_chance_per_generation: f32,
    pub crew: CrewConfig,
    pub failure_risk: FailureRiskConfig,
    pub log_limit: usize,
}

/// Crew roster tunables (GDD §4 Recruit/Train verbs). One post per
/// archetype; recruiting fills a vacancy, training raises the holder's
/// skill toward the archetype cap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewConfig {
    pub starting_posts: Vec<String>,
    pub recruit_cost_credits: i64,
    pub train_cost_credits: i64,
    pub train_skill_gain: u32,
    pub recruit_age_min: u32,
    pub recruit_age_max: u32,
    pub retirement_age: u32,
    /// Security-chief unity recovery only applies below this ceiling.
    pub unity_recovery_ceiling: f32,
}

/// Thresholds and point values for the §5.5 failure-risk formula. Drift and
/// unity apply to every legacy; the rest gate on the matching legacy's
/// tracked counters (see `simulation::legacy::failure_risk`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FailureRiskConfig {
    pub drift_threshold: f32,
    pub drift_points: i32,
    pub unity_threshold: f32,
    pub unity_points: i32,
    pub tradition_threshold: i32,
    pub tradition_points: i32,
    pub body_horror_threshold: u32,
    pub body_horror_points: i32,
    pub dread_threshold: f32,
    pub dread_points: i32,
    pub piracy_threshold: f32,
    pub piracy_points: i32,
    pub at_risk_threshold: i32,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub config: GameConfig,
    pub ship_components: ShipComponentCatalog,
    pub events: DataRegistry<EventTemplate>,
    pub legacies: DataRegistry<LegacyDef>,
    pub contracts: DataRegistry<ContractTemplate>,
    pub dynasty_names: DynastyNamePools,
    pub crew_archetypes: Vec<CrewArchetype>,
    pub texture_manifest: Vec<TextureConfig>,
}

impl GameData {
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            config: load_embedded_json_labeled("game_config", GAME_CONFIG_JSON)?,
            ship_components: load_embedded_json_labeled("ship_components", SHIP_COMPONENTS_JSON)?,
            events: DataRegistry::from_embedded_json(EVENTS_JSON, "id")?,
            legacies: DataRegistry::from_embedded_json(LEGACIES_JSON, "id")?,
            contracts: DataRegistry::from_embedded_json(CONTRACTS_JSON, "id")?,
            dynasty_names: load_embedded_json_labeled("dynasty_names", DYNASTY_NAMES_JSON)?,
            crew_archetypes: load_embedded_json_labeled("crew_archetypes", CREW_ARCHETYPES_JSON)?,
            texture_manifest: load_embedded_json(TEXTURE_MANIFEST_JSON)?,
        })
    }

    /// Registry ids sorted for deterministic iteration (`DataRegistry` is
    /// hash-map backed, so raw iteration order is unstable — never feed it
    /// to the seeded RNG unsorted).
    pub fn sorted_ids<T: Clone>(registry: &DataRegistry<T>) -> Vec<String> {
        let mut ids: Vec<String> = registry.ids().cloned().collect();
        ids.sort();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_data_loads() {
        let data = GameData::load().unwrap();

        assert_eq!(data.config.game_name, "stellar_legacy");
        assert_eq!(data.legacies.len(), 3);
        assert!(data.events.len() >= 4);
        assert!(!data.contracts.is_empty());
        assert_eq!(data.ship_components.hulls.len(), 3);
        assert_eq!(data.crew_archetypes.len(), 7);
        for legacy_id in ["preservers", "adaptors", "wanderers"] {
            assert!(data.legacies.contains(legacy_id));
            assert!(data
                .dynasty_names
                .surnames_by_legacy
                .contains_key(legacy_id));
            assert!(data.dynasty_names.traits_by_legacy.contains_key(legacy_id));
        }
    }

    #[test]
    fn event_categories_all_represented() {
        use events::EventCategory::*;
        let data = GameData::load().unwrap();
        for category in [
            ImmediateCrisis,
            GenerationalChallenge,
            MissionMilestone,
            LegacyMoment,
        ] {
            assert!(
                data.events.iter().any(|(_, e)| e.category == category),
                "no event template for {category:?}"
            );
        }
    }
}
