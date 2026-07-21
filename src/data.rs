//! Embedded game data: config, content registries, and shared delta types.

pub mod contracts;
pub mod crew;
pub mod events;
pub mod factions;
pub mod legacies;
pub mod ship_components;
pub mod subsystems;

use std::collections::HashMap;

use macroquad_toolkit::assets::TextureConfig;
use macroquad_toolkit::data_loader::{
    load_embedded_json, load_embedded_json_labeled, DataRegistry,
};
use serde::{Deserialize, Serialize};

use contracts::ContractTemplate;
use crew::{CrewArchetype, DynastyNamePools};
use events::EventTemplate;
use factions::{FactionConfig, FactionDef};
use legacies::LegacyDef;
use ship_components::ShipComponentCatalog;
use subsystems::{SubsystemDef, SubsystemsConfig};

const GAME_CONFIG_JSON: &str = include_str!("../assets/data/game_config.json");
const TEXTURE_MANIFEST_JSON: &str = include_str!("../assets/data/texture_manifest.json");
const SHIP_COMPONENTS_JSON: &str = include_str!("../assets/ship_components.json");
const EVENTS_JSON: &str = include_str!("../assets/events.json");
const LEGACIES_JSON: &str = include_str!("../assets/legacies.json");
const CONTRACTS_JSON: &str = include_str!("../assets/contracts.json");
const FACTIONS_JSON: &str = include_str!("../assets/factions.json");
const SUBSYSTEMS_JSON: &str = include_str!("../assets/subsystems.json");
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
    /// Spare parts the ship launches with (W1-rescale). A generational voyage
    /// carries a deeper store than the old ~55-yr charters needed.
    pub starting_spare_parts: i64,
    /// Spare parts spent per year keeping the ship maintained (PLAN M4.2).
    /// While parts remain to cover it, yearly wear is eased by
    /// `maintenance_decay_relief`; once the stores run dry, wear is full rate.
    pub parts_upkeep_per_year: i64,
    /// Fraction of a year's hull/life-support decay avoided while the ship is
    /// maintained (0 = no relief, 0.4 = 40% less wear that year).
    pub maintenance_decay_relief: f32,
    pub generation_interval_years: u32,
    pub leader_retirement_age: u32,
    pub heir_min_age: u32,
    pub heir_max_age: u32,
    pub member_max_age: u32,
    pub event_chance_base: f32,
    pub event_chance_cap: f32,
    /// Chance a legacy dilemma confronts each new generation (GDD §5.5).
    pub dilemma_chance_per_generation: f32,
    /// Founding-faction tunables (W7).
    pub factions: FactionConfig,
    /// Pre-launch provisioning + fuel-as-consumable tunables (W4).
    pub provisioning: ProvisioningConfig,
    /// First-voyage tutorial content: the drydock hint and PREP checklist.
    pub tutorial: TutorialConfig,
    /// Ship-subsystem knowledge/training tunables (W5).
    pub subsystems: SubsystemsConfig,
    /// Seeded-campaign-skeleton beat pools + era layering (content-depth).
    pub campaign_skeleton: CampaignSkeletonConfig,
    /// Generational obituary/succession/coming-of-age flavor pools (content-depth
    /// voice iteration).
    pub flavor: FlavorConfig,
    pub crew: CrewConfig,
    pub failure_risk: FailureRiskConfig,
    pub ship: ShipConfig,
    /// Per-year population drift over a voyage (PLAN M4.1).
    pub voyage_drift: VoyageDrift,
    /// Field-vs-port repair tunables (PLAN M4.3).
    pub repair: RepairConfig,
    /// Gating for installing salvaged parts underway (PLAN M4.4).
    pub field_install: FieldInstallConfig,
    /// Commission-a-new-ship tunables (PLAN M4.5).
    pub commission: CommissionConfig,
    /// Heritage tiers (GDD §7), ascending by `min_renown`. The highest tier a
    /// new dynasty's accumulated Chronicle renown clears grants its bonus.
    pub heritage: Vec<HeritageTier>,
    pub log_limit: usize,
}

/// One heritage tier: the renown needed to reach it and the head start it
/// grants a new campaign (`simulation`/`heritage`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeritageTier {
    pub min_renown: i64,
    pub name: String,
    #[serde(default)]
    pub credits: i64,
    #[serde(default)]
    pub influence: i64,
    #[serde(default)]
    pub tradition: i32,
}

/// Ship-loadout tunables (PLAN item 3). The installed components' aggregated
/// stats scale a yearly production bonus and fuel regeneration
/// (`simulation::ship`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ShipConfig {
    /// Credits per point of aggregate engine/hull speed (faster trade runs).
    pub credits_per_speed: i64,
    /// Minerals per point of aggregate cargo (bigger holds haul more).
    pub minerals_per_cargo: f32,
    /// Fuel fraction restored per point of aggregate fuel_regen each year.
    pub fuel_regen_per_point: f32,
    /// Bonus contract progress-years added per point of aggregate speed each
    /// year (boosts milestones/score, not the duration).
    pub contract_progress_per_speed: f32,
    /// Success-chance bonus per point of aggregate combat on Wanderer dilemmas
    /// (firepower backs the confrontation).
    pub combat_dilemma_odds_per_point: f32,
    /// Ceiling on an effective dilemma success chance after the combat bonus.
    pub dilemma_odds_cap: f32,
}

/// Per-year population drift over a voyage (PLAN M4.1): a long mission changes
/// the people, not just the ship. Applied every year in `simulation::tick`,
/// deterministic (no RNG) and clamped by `PopulationState::apply`. The identity
/// terms (adaptation / cultural_drift / legacy_loyalty) are scaled by a
/// per-legacy multiplier so Adaptors change fastest and Preservers slowest; the
/// voyage strain on morale/unity is universal (not scaled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoyageDrift {
    pub adaptation_per_year: f32,
    pub cultural_drift_per_year: f32,
    pub legacy_loyalty_per_year: f32,
    pub morale_strain_per_year: f32,
    pub unity_strain_per_year: f32,
    /// Legacy id → magnitude multiplier for the identity terms.
    pub legacy_multipliers: HashMap<String, f32>,
}

/// Field-vs-port repair tunables (PLAN M4.3). Underway, `field_repair` patches
/// a stat by `field_gain` up to `field_ceiling` (never pristine) for
/// `field_parts_cost` spare parts + `field_minerals_cost` minerals. In port,
/// `full_repair` restores everything to whole for `full_credits_cost` +
/// `full_minerals_cost` and tops parts back up to `full_parts_restock`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RepairConfig {
    pub field_ceiling: f32,
    pub field_gain: f32,
    pub field_parts_cost: i64,
    pub field_minerals_cost: i64,
    pub full_credits_cost: i64,
    pub full_minerals_cost: i64,
    pub full_parts_restock: i64,
}

/// Gating for fitting a salvaged component underway (PLAN M4.4). At port any
/// part installs freely; in the black it needs a `field_installable` part, an
/// engineer at `skill_required`, and `parts_cost` spare parts + `minerals_cost`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FieldInstallConfig {
    pub skill_required: u32,
    pub parts_cost: i64,
    pub minerals_cost: i64,
}

/// Commission-a-new-ship tunables (PLAN M4.5). Commissioning a hull costs the
/// hull's own catalog price plus this premium (a whole fresh vessel), fully
/// refits the ship, and lifts morale/unity — a new ship renews hope. It never
/// resets the population's drift; the people carry across.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CommissionConfig {
    pub premium_credits: i64,
    pub premium_minerals: i64,
    pub hope_morale: f32,
    pub hope_unity: f32,
}

/// Crew roster tunables (GDD §4 Recruit/Train verbs). One post per
/// Provisioning + fuel tunables (W4). Fuel is a consumable voyage store burned
/// during Travel; an empty tank stalls travel and doubles systems decay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProvisioningConfig {
    /// Fuel fraction burned each Travel-phase month.
    pub fuel_burn_per_travel_month: f32,
    /// Credits to refuel one whole fuel point (a full 0→1 tank).
    pub fuel_cost_credits_per_point: i64,
    /// Hull/life-support decay multiplier for a year in which the tank ran dry.
    pub no_fuel_decay_multiplier: f32,
    /// Credits per spare part when stocking up in drydock (PREP screen).
    pub part_cost_credits: i64,
}

/// Generational-flavor line pools (content-depth voice iteration): the
/// most-repeated text in the game — the obituary, succession, and coming-of-age
/// lines that fire every generation — moved out of Rust so they can vary instead
/// of reading the same three strings a dozen times a voyage. Lines are picked
/// deterministically (by generation index, no RNG), so a seed still replays
/// exactly. `{name}` / `{generation}` / `{births}` placeholders are substituted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlavorConfig {
    /// A dynasty member laid to rest. Placeholder: `{name}`.
    pub obituary: Vec<String>,
    /// A new head of the dynasty takes over. Placeholder: `{name}`.
    pub succession: Vec<String>,
    /// A new cohort comes of age. Placeholders: `{generation}`, `{births}`.
    pub coming_of_age: Vec<String>,
}

impl FlavorConfig {
    /// Deterministic pick from `pool` by rotating index `n`, with `{name}`
    /// substituted. Returns `None` only when the pool is empty.
    pub fn line_with_name(pool: &[String], n: usize, name: &str) -> Option<String> {
        (!pool.is_empty()).then(|| pool[n % pool.len()].replace("{name}", name))
    }
}

/// Seeded-campaign-skeleton tunables (content-depth iteration): the phase→family
/// beat pools, moved out of Rust so the campaign's shape is data like everything
/// else, plus era layering that tints founding-era and homecoming-era beats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignSkeletonConfig {
    /// One beat per this many months of mission duration.
    pub months_per_window: u32,
    /// No beats before this many months into the voyage.
    pub skip_months: u32,
    /// Family pools drawn from by the phase a beat lands in.
    pub travel_pool: Vec<String>,
    pub operation_pool: Vec<String>,
    pub return_pool: Vec<String>,
    /// Families eligible in any phase, always added to the draw.
    pub any_pool: Vec<String>,
    /// Extra families layered in for beats in the first `early_fraction` of the
    /// voyage (founding-era texture) and the last `late_fraction`→end
    /// (homecoming-era texture).
    pub early_pool: Vec<String>,
    pub late_pool: Vec<String>,
    pub early_fraction: f32,
    pub late_fraction: f32,
}

/// One step of the first-voyage checklist. The `id` binds it to a completion
/// check in the PREP screen; label and tip are authored content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialStep {
    pub id: String,
    pub label: String,
    pub tip: String,
}

/// First-voyage tutorial content. Shown only until the Chronicle records a
/// mission (or the player dismisses it); all text is data, per the hard rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialConfig {
    /// One-line hint over the drydock charter list on a first voyage.
    pub drydock_hint: String,
    /// The same line's everyday text once the tutorial is over.
    pub drydock_refit_hint: String,
    /// Ordered pre-launch checklist steps for the PREP screen.
    pub steps: Vec<TutorialStep>,
}

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
    pub factions: DataRegistry<FactionDef>,
    pub subsystems: DataRegistry<SubsystemDef>,
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
            factions: DataRegistry::from_embedded_json(FACTIONS_JSON, "id")?,
            subsystems: DataRegistry::from_embedded_json(SUBSYSTEMS_JSON, "id")?,
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
    fn flavor_lines_rotate_deterministically_and_substitute_name() {
        let pool = vec!["A {name}".to_string(), "B {name}".to_string()];
        // Rotates by index, wraps, and substitutes — no RNG, so a seed replays.
        assert_eq!(
            FlavorConfig::line_with_name(&pool, 0, "Vale").unwrap(),
            "A Vale"
        );
        assert_eq!(
            FlavorConfig::line_with_name(&pool, 1, "Vale").unwrap(),
            "B Vale"
        );
        assert_eq!(
            FlavorConfig::line_with_name(&pool, 2, "Vale").unwrap(),
            "A Vale"
        );
        assert!(FlavorConfig::line_with_name(&[], 0, "Vale").is_none());
    }

    #[test]
    fn embedded_data_loads() {
        let data = GameData::load().unwrap();

        assert_eq!(data.config.game_name, "stellar_legacy");
        assert_eq!(data.legacies.len(), 3);
        assert!(data.events.len() >= 4);
        assert!(
            data.contracts.len() >= 10,
            "§8 target was 6-8 contracts; the pool has since grown"
        );
        // Charter tiering (PLAN M4.8): some charters gate behind renown, some
        // are available from the founding.
        assert!(
            data.contracts.iter().any(|(_, c)| c.min_renown > 0),
            "some charters should unlock with renown"
        );
        assert!(
            data.contracts.iter().any(|(_, c)| c.min_renown == 0),
            "some charters should be available from the founding"
        );
        // W1-rescale: every charter is now a generational voyage (>= 300 yr).
        // W2: authored phases sum exactly to the duration, only Travel/Operation/
        // Return kinds, at least one Operation segment, and a real objective.
        use contracts::ContractPhase;
        for (id, c) in data.contracts.iter() {
            assert!(
                c.target_duration_years >= 300,
                "charter '{id}' must be a generational voyage (>= 300 yr), is {}",
                c.target_duration_years
            );
            let phase_years: u32 = c.phases.iter().map(|p| p.years).sum();
            assert_eq!(
                phase_years, c.target_duration_years,
                "charter '{id}' phase years {phase_years} must sum to its duration {}",
                c.target_duration_years
            );
            for phase in &c.phases {
                assert!(
                    matches!(
                        phase.kind,
                        ContractPhase::Travel | ContractPhase::Operation | ContractPhase::Return
                    ),
                    "charter '{id}' has an invalid authored phase kind {:?}",
                    phase.kind
                );
            }
            assert!(
                c.phases.iter().any(|p| p.kind == ContractPhase::Operation),
                "charter '{id}' must have at least one Operation segment"
            );
            assert!(
                c.objective_target > 0.0,
                "charter '{id}' must have a positive objective target"
            );
        }
        // Salvage pool (PLAN M4.4): several event outcomes drop a found part,
        // and every granted id must resolve to a real component.
        let salvage_grants: Vec<&String> = data
            .events
            .iter()
            .flat_map(|(_, e)| e.outcomes.iter())
            .filter_map(|o| o.grant_component.as_ref())
            .collect();
        assert!(
            salvage_grants.len() >= 4,
            "expected >= 4 salvage-granting outcomes, found {}",
            salvage_grants.len()
        );
        for id in salvage_grants {
            assert!(
                data.ship_components.find_any(id).is_some(),
                "event grant_component '{id}' must be a real ship component"
            );
        }
        // Content-depth charter↔event coupling: every charter-tag an event gates
        // on must exist on at least one charter, or the event can never fire.
        let charter_tags: std::collections::HashSet<&String> = data
            .contracts
            .iter()
            .flat_map(|(_, c)| c.tags.iter())
            .collect();
        for (id, e) in data.events.iter() {
            for tag in &e.requires_charter_tag {
                assert!(
                    charter_tags.contains(tag),
                    "event '{id}' requires charter tag '{tag}' no charter carries"
                );
            }
            // Content-depth faction↔event coupling: every faction an event gates
            // on must be a real, authored faction.
            for fid in std::iter::once(&e.requires_dominant_faction)
                .filter(|f| !f.is_empty())
                .chain(e.requires_factions_aboard.iter())
            {
                assert!(
                    data.factions.get(fid).is_some(),
                    "event '{id}' gates on unknown faction '{fid}'"
                );
            }
            // Content-depth subsystem↔event coupling: knowledge gates and
            // outcome subsystem deltas must name real subsystems.
            for sid in e.knowledge_below.iter().map(|g| &g.id).chain(
                e.outcomes
                    .iter()
                    .flat_map(|o| o.subsystem_deltas.iter().map(|d| &d.id)),
            ) {
                assert!(
                    data.subsystems.get(sid).is_some(),
                    "event '{id}' references unknown subsystem '{sid}'"
                );
            }
        }
        // Content-depth consequence chains: every tag a payoff event gates on
        // (`requires_consequence`) must be produced by some outcome's
        // `long_term_consequences`, or the payoff can never fire (typo guard).
        let produced: std::collections::HashSet<&String> = data
            .events
            .iter()
            .flat_map(|(_, e)| e.outcomes.iter())
            .flat_map(|o| o.long_term_consequences.iter())
            .collect();
        for (id, e) in data.events.iter() {
            for tag in &e.requires_consequence {
                assert!(
                    produced.contains(tag),
                    "event '{id}' gates on consequence '{tag}' no outcome records"
                );
            }
        }
        // W7: six authored founding factions, ideology within [-1, 1]. The
        // registry keys on id, so a count of six also proves the ids are unique.
        assert_eq!(data.factions.len(), 6, "six founding factions");
        for (id, faction) in data.factions.iter() {
            assert!(
                (-1.0..=1.0).contains(&faction.ideology),
                "faction '{id}' ideology out of range: {}",
                faction.ideology
            );
        }

        // W5: six subsystems load; each non-empty buffered family is one of the
        // canonical W6 family strings; tiers are well-formed (3, positive cost).
        let canonical_families: std::collections::HashSet<&str> = [
            "exploration_first_contact",
            "diplomacy",
            "engineering",
            "biology_medical",
            "science_anomaly",
            "survival",
            "mystery",
            "comedy",
            "ethics",
            "legacy_drift",
        ]
        .into_iter()
        .collect();
        assert_eq!(data.subsystems.len(), 6, "six ship subsystems");
        for (id, sub) in data.subsystems.iter() {
            if !sub.buffers_family.is_empty() {
                assert!(
                    canonical_families.contains(sub.buffers_family.as_str()),
                    "subsystem '{id}' buffers a non-canonical family '{}'",
                    sub.buffers_family
                );
            }
            assert_eq!(
                sub.tiers.len(),
                3,
                "subsystem '{id}' has three upgrade tiers"
            );
            for tier in &sub.tiers {
                assert!(
                    tier.cost.credits > 0,
                    "subsystem '{id}' tier cost must be positive"
                );
            }
        }

        assert_eq!(data.ship_components.hulls.len(), 5);
        assert_eq!(data.ship_components.engines.len(), 5);
        assert_eq!(data.ship_components.weapons.len(), 5);
        assert_eq!(data.crew_archetypes.len(), 7);
        // Doubled name pools (§8): 50 given names, 20 surnames + 10 traits
        // per legacy.
        assert!(data.dynasty_names.given_names.len() >= 50);
        for legacy_id in ["preservers", "adaptors", "wanderers"] {
            assert!(data.legacies.contains(legacy_id));
            let surnames = &data.dynasty_names.surnames_by_legacy[legacy_id];
            let traits = &data.dynasty_names.traits_by_legacy[legacy_id];
            assert!(
                surnames.len() >= 20,
                "{legacy_id} surnames: {}",
                surnames.len()
            );
            assert!(traits.len() >= 10, "{legacy_id} traits: {}", traits.len());
            // Each legacy carries its defining dilemmas (§8 target 6; the
            // pool has since been deepened past it).
            let dilemmas = data.legacies.get(legacy_id).unwrap().dilemmas.len();
            assert!(
                dilemmas >= 8,
                "{legacy_id} should have >= 8 dilemmas, has {dilemmas}"
            );
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
            // Every category is well represented (§8 M3 target is 30+ total).
            let count = data
                .events
                .iter()
                .filter(|(_, e)| e.category == category)
                .count();
            assert!(
                count >= 11,
                "expected >= 11 event templates for {category:?}, found {count}"
            );
        }
        // §8 M3 target is 30+; the pool has since grown well past it.
        assert!(
            data.events.len() >= 46,
            "expected >= 46 event templates, found {}",
            data.events.len()
        );
        // Content-depth campaign-skeleton coupling: every family a beat pool can
        // draw must have authored events, or a beat could land on an empty pool.
        let families: std::collections::HashSet<&String> =
            data.events.iter().map(|(_, e)| &e.family).collect();
        let sk = &data.config.campaign_skeleton;
        for fam in sk
            .travel_pool
            .iter()
            .chain(&sk.operation_pool)
            .chain(&sk.return_pool)
            .chain(&sk.any_pool)
            .chain(&sk.early_pool)
            .chain(&sk.late_pool)
        {
            assert!(
                families.contains(fam),
                "campaign_skeleton pool family '{fam}' has no events"
            );
        }
        // Content-depth voice: every generational-flavor pool must be non-empty
        // (or a generation turns over in silence) and carry its placeholder.
        let fl = &data.config.flavor;
        assert!(
            fl.obituary.iter().any(|s| s.contains("{name}")),
            "obituary flavor needs a {{name}} line"
        );
        assert!(
            fl.succession.iter().any(|s| s.contains("{name}")),
            "succession flavor needs a {{name}} line"
        );
        assert!(
            !fl.coming_of_age.is_empty(),
            "coming_of_age flavor must not be empty"
        );
    }

    #[test]
    fn every_event_is_tagged_and_families_are_filled() {
        use crate::data::contracts::ContractPhase;
        use std::collections::HashMap;
        let data = GameData::load().unwrap();
        let canonical: std::collections::HashSet<&str> = [
            "exploration_first_contact",
            "diplomacy",
            "engineering",
            "biology_medical",
            "science_anomaly",
            "survival",
            "mystery",
            "comedy",
            "ethics",
            "legacy_drift",
        ]
        .into_iter()
        .collect();

        let mut counts: HashMap<String, usize> = HashMap::new();
        for (id, e) in data.events.iter() {
            assert!(!e.family.is_empty(), "event '{id}' has no family (W6)");
            assert!(
                canonical.contains(e.family.as_str()),
                "event '{id}' family '{}' is not one of the canonical ten",
                e.family
            );
            for phase in &e.phases {
                assert!(
                    matches!(
                        phase,
                        ContractPhase::Travel | ContractPhase::Operation | ContractPhase::Return
                    ),
                    "event '{id}' has a non-voyage phase gate {phase:?}"
                );
            }
            *counts.entry(e.family.clone()).or_default() += 1;
        }

        assert!(
            data.events.len() >= 60,
            "W6 wants >= 60 templates, found {}",
            data.events.len()
        );
        for family in &canonical {
            let n = counts.get(*family).copied().unwrap_or(0);
            assert!(
                n >= 6,
                "family '{family}' has only {n} templates (W6 wants >= 6)"
            );
        }
    }

    #[test]
    fn tutorial_steps_cover_the_launch_flow() {
        let data = GameData::load().unwrap();
        let tutorial = &data.config.tutorial;
        assert!(!tutorial.drydock_hint.trim().is_empty());
        assert!(!tutorial.drydock_refit_hint.trim().is_empty());
        // The PREP checklist binds these ids to completion checks — the
        // authored steps must match them exactly, in launch order.
        let ids: Vec<&str> = tutorial.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "choose_charter",
                "stock_food",
                "stock_parts",
                "fuel_tanks",
                "launch"
            ],
            "tutorial steps must match the PREP checklist's known ids"
        );
        for step in &tutorial.steps {
            assert!(!step.label.trim().is_empty(), "step '{}' label", step.id);
            assert!(!step.tip.trim().is_empty(), "step '{}' tip", step.id);
        }
    }

    #[test]
    fn a_new_ship_sails_provisioned_for_a_starter_charter() {
        // A new player should be able to fly a renown-0 charter without
        // shopping first: the founding stores cover the shortest one whole.
        let data = GameData::load().unwrap();
        let config = &data.config;
        let starter_years = data
            .contracts
            .iter()
            .filter(|(_, c)| c.min_renown == 0)
            .map(|(_, c)| c.target_duration_years)
            .min()
            .expect("at least one renown-0 charter");
        let food_need = (config.starting_population as f32
            * config.food_per_person_per_year
            * starter_years as f32)
            .ceil() as i64;
        assert!(
            config.starting_resources.food >= food_need,
            "founding food {} must cover a {starter_years}-yr starter charter ({food_need})",
            config.starting_resources.food
        );
        assert!(
            config.starting_spare_parts >= config.parts_upkeep_per_year * starter_years as i64,
            "founding parts {} must cover {starter_years} years of upkeep",
            config.starting_spare_parts
        );
    }
}
