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
/// Event templates, split per `family` (content-depth): one file per family
/// under `assets/events/` so no single content file grows unwieldy. Embedded via
/// `include_str!` (WASM-safe, same as before); merged into one registry at load
/// with a hard duplicate-id guard. Adding a new family = add one line here.
const EVENT_FILES: &[(&str, &str)] = &[
    (
        "biology_medical",
        include_str!("../assets/events/biology_medical.json"),
    ),
    ("comedy", include_str!("../assets/events/comedy.json")),
    ("diplomacy", include_str!("../assets/events/diplomacy.json")),
    (
        "engineering",
        include_str!("../assets/events/engineering.json"),
    ),
    ("ethics", include_str!("../assets/events/ethics.json")),
    (
        "exploration_first_contact",
        include_str!("../assets/events/exploration_first_contact.json"),
    ),
    (
        "legacy_drift",
        include_str!("../assets/events/legacy_drift.json"),
    ),
    ("mystery", include_str!("../assets/events/mystery.json")),
    (
        "science_anomaly",
        include_str!("../assets/events/science_anomaly.json"),
    ),
    ("survival", include_str!("../assets/events/survival.json")),
];
const LEGACIES_JSON: &str = include_str!("../assets/legacies.json");
const CONTRACTS_JSON: &str = include_str!("../assets/contracts.json");
const FACTIONS_JSON: &str = include_str!("../assets/factions.json");
const SUBSYSTEMS_JSON: &str = include_str!("../assets/subsystems.json");
const DYNASTY_NAMES_JSON: &str = include_str!("../assets/dynasty_names.json");
const CREW_ARCHETYPES_JSON: &str = include_str!("../assets/crew_archetypes.json");

/// Signed per-resource change used by event outcomes, costs, and rewards.
/// Also doubles as an absolute amount set (e.g. starting resources).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ShipDelta {
    pub hull_integrity: f32,
    pub life_support: f32,
    pub fuel: f32,
    pub spare_parts: i32,
}

/// Signed change to colony-scale population stats.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
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
    /// Food store below which a year counts as *lean* (content-depth provisioning
    /// round 13): distinct from the near-famine `low_food_threshold`, this is the
    /// "not comfortably stocked" line whose sustained crossing drives `lean_food_years`
    /// — the state that separates a bad year from a bad generation. 0 = disabled.
    #[serde(default)]
    pub lean_food_threshold: i64,
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
    /// How much the *dominant* faction's ideology bends the identity drift
    /// (content-depth factions round 9): the yearly identity terms scale by
    /// `1 + dominant_ideology_scale * ideology`, so a tech-embracing majority
    /// (ideology > 0) drifts the people from the founders faster, a
    /// tradition-bound one (< 0) slower. 0 = who runs the ship has no effect.
    /// Kept gentle so identity still moves in the same direction whoever leads.
    #[serde(default)]
    pub dominant_ideology_scale: f32,
    /// How much a well-kept culture archive resists the people forgetting the
    /// founders (content-depth subsystems round 10): the *cultural* drift terms
    /// (cultural_drift, legacy_loyalty fade) scale by
    /// `1 - archive_drift_resistance * education_culture_knowledge`, so a ship
    /// that keeps its founding memory vivid drifts culturally slower — but its
    /// bodies still adapt to the ship regardless. 0 = the archive doesn't matter.
    #[serde(default)]
    pub archive_drift_resistance: f32,
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
    /// A crew member stands down from their post at a generation turnover
    /// (content-depth voice round 5). Fires once per retiring holder — several a
    /// generation — so it needs pool variety or it is a repetition tell.
    /// Placeholder: `{name}`. Empty falls back to the built-in line.
    #[serde(default)]
    pub retirement: Vec<String>,
    /// The dynasty ends with no heir (content-depth voice round 5): the tragic
    /// counterpart to `homecoming`, indexed by generation. Empty falls back to
    /// the built-in line so the ending is never blank.
    #[serde(default)]
    pub extinction: Vec<String>,
    /// A starving year (content-depth voice round 6): fires once per *year* the
    /// larder is empty, so a multi-year famine needs variety or it reprints one
    /// line. Placeholder: `{losses}`. Indexed by year; empty falls back.
    #[serde(default)]
    pub famine: Vec<String>,
    /// A year coasting on a dry tank (content-depth voice round 6): like famine,
    /// fires once per stalled year. Indexed by year; empty falls back.
    #[serde(default)]
    pub fuel_stall: Vec<String>,
    /// A crew officer takes up a post (content-depth voice round 7): the positive
    /// twin of `retirement`, fired whenever a vacancy is filled — repeatedly
    /// across a voyage as posts turn over — so it needs variety and the post's
    /// human name, not the raw archetype id. Placeholders `{name}`, `{post}`.
    /// Indexed by crew id (deterministic). Empty falls back to the built-in line.
    #[serde(default)]
    pub appointment: Vec<String>,
    /// An officer completes a training program (content-depth voice round 7): a
    /// repeatable drydock verb, so it needs variety over the flat bracketed
    /// skill number. Placeholders `{name}`, `{post}`, `{skill}`. Indexed by the
    /// new skill; empty falls back to the built-in line.
    #[serde(default)]
    pub training: Vec<String>,
    /// A people crossing *into* restlessness (content-depth voice round 8): the
    /// otherwise-silent approval meter finally speaks, so the player feels a
    /// faction souring toward its withdrawal. Placeholder `{name}` (the people's
    /// log name). Indexed by year; empty falls back to silence.
    #[serde(default)]
    pub faction_souring: Vec<String>,
    /// A people crossing *into* contentment (content-depth voice round 8): the
    /// positive twin, when goodwill has climbed high. Placeholder `{name}`.
    #[serde(default)]
    pub faction_warming: Vec<String>,
    /// The *whole ship's* morale crossing *into* a heavy band (content-depth voice
    /// round 11): the collective parallel to `faction_souring` — where that voices
    /// one people souring, this voices the decks as a whole going grim. No name;
    /// indexed by year; empty falls back to silence.
    #[serde(default)]
    pub ship_mood_darkening: Vec<String>,
    /// The whole ship's morale crossing *into* a light band (content-depth voice
    /// round 11): the positive twin, the decks lifting together. No name; indexed
    /// by year; empty falls back to silence.
    #[serde(default)]
    pub ship_mood_lifting: Vec<String>,
    /// A subsystem patched back toward working order (content-depth voice round 9):
    /// the field-repair verb fires repeatedly across a voyage, so the flat line it
    /// used needs variety. Placeholder `{name}` (the module). Indexed by the month
    /// clock; empty falls back to the built-in line.
    #[serde(default)]
    pub subsystem_repair: Vec<String>,
    /// A new cohort trained up on a subsystem (content-depth voice round 9): the
    /// knowledge-training verb, likewise repeatable. Placeholder `{name}`. Indexed
    /// by the month clock; empty falls back to the built-in line.
    #[serde(default)]
    pub subsystem_training: Vec<String>,
    /// Atmospheric "life aboard" lines surfaced during long event-less stretches
    /// (content-depth voice round 2), so the passing centuries read as lived-in
    /// rather than empty. Dated by the log itself, indexed by year (no RNG).
    #[serde(default)]
    pub ambient: Vec<String>,
    /// Ambient lines for a *far-drifted* ship (content-depth voice round 10): once
    /// cultural drift crosses `ambient_drift_threshold`, the quiet stretches draw
    /// from this pool instead — the same lived-in texture gone alien, so the log
    /// itself reflects how far the people have come from the founders. Empty =
    /// always use `ambient`.
    #[serde(default)]
    pub ambient_drifted: Vec<String>,
    /// Cultural-drift level at or past which quiet stretches read from
    /// `ambient_drifted`. 0 with a non-empty drifted pool means always drifted.
    #[serde(default)]
    pub ambient_drift_threshold: f32,
    /// Ambient lines for a *hollowed-out* ship (content-depth voice round 12): once
    /// the crew has thinned to `ambient_population_threshold` or fewer, the quiet
    /// stretches draw from this pool — the same lived-in texture gone sparse and
    /// echoing, corridors built for thousands walked by hundreds, so the log
    /// reflects how empty the ship has become. Takes precedence over `ambient_drifted`
    /// (emptiness is the louder note in a silence). Empty = always use the others.
    #[serde(default)]
    pub ambient_hollow: Vec<String>,
    /// Crew headcount at or below which quiet stretches read from `ambient_hollow`
    /// (content-depth voice round 12). An absolute count (founding is ~1000).
    #[serde(default)]
    pub ambient_population_threshold: u32,
    /// Ambient lines for a *long-hungry* ship (content-depth voice round 13): once
    /// the food store has sat below the lean line for `ambient_lean_years_threshold`
    /// years or more (`SimState.lean_food_years`), the quiet stretches draw from this
    /// pool — the lived-in texture gone thin and rationed, the daily preoccupation
    /// with the next plate. Takes precedence over `ambient_hollow` (a sustained
    /// hunger is the most immediate lived condition). Empty = always use the others.
    #[serde(default)]
    pub ambient_lean: Vec<String>,
    /// Consecutive lean years at or past which quiet stretches read from
    /// `ambient_lean` (content-depth voice round 13).
    #[serde(default)]
    pub ambient_lean_years_threshold: u32,
    /// Years of event-less quiet between ambient lines (0 = ambient off).
    #[serde(default)]
    pub ambient_gap_years: u32,
    /// Phase-transition line pools keyed by phase (snake_case: travel, operation,
    /// return, completion, preparation), content-depth voice round 3. Indexed by
    /// how many times that phase has been entered this voyage, so a double-hop's
    /// second departure/arrival reads differently from the first. An empty or
    /// missing pool falls back to the built-in line.
    #[serde(default)]
    pub phase_lines: HashMap<String, Vec<String>>,
    /// Homecoming prose pools keyed by mission success level (snake_case:
    /// complete, partial, pyrrhic, failure), content-depth voice round 4. The
    /// end of a centuries-long voyage is the campaign's emotional climax; this
    /// gives it level-specific prose instead of one flat mechanical line.
    /// Placeholders `{years}`, `{generation}`. Empty or missing pool falls back
    /// to the built-in line so the log is never blank.
    #[serde(default)]
    pub homecoming: HashMap<String, Vec<String>>,
}

impl FlavorConfig {
    /// Deterministic pick from `pool` by rotating index `n`, with `{name}`
    /// substituted. Returns `None` only when the pool is empty.
    pub fn line_with_name(pool: &[String], n: usize, name: &str) -> Option<String> {
        (!pool.is_empty()).then(|| pool[n % pool.len()].replace("{name}", name))
    }

    /// Like `line_with_name`, additionally substituting `{post}` (the officer's
    /// human post name) — for crew-turnover lines that name both the person and
    /// the post they take or leave. `None` only when the pool is empty.
    pub fn line_with_name_post(
        pool: &[String],
        n: usize,
        name: &str,
        post: &str,
    ) -> Option<String> {
        (!pool.is_empty()).then(|| {
            pool[n % pool.len()]
                .replace("{name}", name)
                .replace("{post}", post)
        })
    }

    /// Homecoming line for a mission that ended at `level_key` (the success
    /// level, snake_case), indexed deterministically by `n` (the generation) so
    /// a seed replays the same line, with `{years}`/`{generation}` substituted.
    /// `None` when no pool is authored for that level — the caller keeps its
    /// built-in line.
    pub fn homecoming_line(
        &self,
        level_key: &str,
        n: usize,
        years: u32,
        generation: u32,
    ) -> Option<String> {
        let pool = self.homecoming.get(level_key)?;
        (!pool.is_empty()).then(|| {
            pool[n % pool.len()]
                .replace("{years}", &years.to_string())
                .replace("{generation}", &generation.to_string())
        })
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
    /// Extra families layered into beats in the deep middle of the voyage
    /// (between `early_fraction` and `late_fraction`) — the era no living soul
    /// remembers launching into and none expects to see the end of, when the
    /// ship is the only world anyone has known (content-depth round 4). Empty =
    /// no mid-era tint.
    #[serde(default)]
    pub mid_pool: Vec<String>,
    /// Cultural-drift thresholds (ascending) that each fire one beat the first
    /// time the voyage crosses them (content-depth round 2). This is how the
    /// signature Long-Term Expedition beats read as *consequences of the long
    /// voyage* — the people having drifted far enough — rather than random
    /// rolls. Empty = no drift beats.
    #[serde(default)]
    pub drift_beats: Vec<f32>,
    /// The family a drift-threshold beat draws from.
    #[serde(default)]
    pub drift_beat_family: String,
    /// Adaptation thresholds (ascending), the physiological/instinctive parallel
    /// to `drift_beats` (content-depth round 3): each fires one beat the first
    /// time the people's `adaptation` crosses it — the descendants growing suited
    /// to the ship in body and habit. Empty = no adaptation beats.
    #[serde(default)]
    pub adaptation_beats: Vec<f32>,
    /// The family an adaptation-threshold beat draws from.
    #[serde(default)]
    pub adaptation_beat_family: String,
    /// Dead-air backstop (content-depth round 5): the most years the voyage may
    /// pass with no event before the skeleton *forces* one. Long eventless
    /// stretches are a content-coverage bug, not a mercy — beyond this gap a beat
    /// is guaranteed. 0 = no backstop.
    #[serde(default)]
    pub dead_air_years: u32,
    /// Families a forced dead-air beat may draw from (one picked via state RNG,
    /// so it stays deterministic). Must be non-empty when `dead_air_years` > 0.
    #[serde(default)]
    pub dead_air_pool: Vec<String>,
    /// Cohesion-collapse thresholds (content-depth round 6): the *descending*
    /// mirror of `drift_beats`/`adaptation_beats`. As the people's `unity` falls
    /// to or below each threshold (thresholds authored high→low), a beat is
    /// forced — the ship coming apart surfaces its own reckoning rather than
    /// waiting on a random roll. Empty = no crisis beats.
    #[serde(default)]
    pub crisis_beats: Vec<f32>,
    /// The family a cohesion-collapse crisis beat draws from.
    #[serde(default)]
    pub crisis_beat_family: String,
    /// Recovery threshold (content-depth round 13): the crisis beat's *hopeful
    /// mirror*. Once the ship has fractured (a crisis beat has fired) and its
    /// `unity` then climbs back to or above this, a beat is forced — the mending, a
    /// ship pulling itself back from the brink — and the crisis counter is reset so
    /// a relapse re-arms the collapse beats. Set well above the crisis thresholds
    /// for hysteresis (the band between neither fires). 0 = no recovery beat.
    #[serde(default)]
    pub recovery_beat_threshold: f32,
    /// The family a recovery/mending beat draws from.
    #[serde(default)]
    pub recovery_beat_family: String,
    /// Flourishing thresholds (content-depth round 8): the *positive* pole of the
    /// crisis beat. As the people's `morale` climbs to or past each threshold
    /// (authored low→high) a beat is forced — a thriving ship generates its own
    /// golden age, so good stewardship surfaces its own beats, not only decline.
    /// Empty = no flourish beats.
    #[serde(default)]
    pub flourish_beats: Vec<f32>,
    /// The family a golden-age flourish beat draws from.
    #[serde(default)]
    pub flourish_beat_family: String,
    /// Depopulation thresholds (content-depth round 12): the crew's *headcount*
    /// finally gets a beat — the one major state dimension none watched. As the
    /// population falls to or below each fraction of its *founding* size (authored
    /// high→low, e.g. 0.6/0.4/0.25 of the launch thousands), a beat is forced — the
    /// sealed ship's defining slow tragedy, the decks thinning across the centuries,
    /// marked at its stages. Campaign-scoped (fires once per fraction a voyage, not
    /// per contract). Empty = no depopulation beats.
    #[serde(default)]
    pub depopulation_beats: Vec<f32>,
    /// The family a crew-thinning depopulation beat draws from.
    #[serde(default)]
    pub depopulation_beat_family: String,
    /// Objective-progress thresholds (content-depth round 9): the first pacing
    /// keyed to *the mission itself* rather than time or an identity stat. As the
    /// active charter's `objective_fraction` crosses each (authored low→high) a
    /// beat is forced — the crew's bond to a purpose most of them will not live
    /// to see completed, marked at its milestones. Empty = no objective beats.
    #[serde(default)]
    pub objective_beats: Vec<f32>,
    /// The family a mission-progress objective beat draws from.
    #[serde(default)]
    pub objective_beat_family: String,
    /// Homecoming beat family (content-depth round 10): the first beat keyed to a
    /// voyage *phase* rather than a stat, time, or the objective. Once the charter
    /// turns for home (enters its Return leg) a single beat is forced from this
    /// family — the climactic identity reckoning the doc names, a generation
    /// meeting a homeport that still remembers the founders it no longer resembles.
    /// Empty = no homecoming beat.
    #[serde(default)]
    pub homecoming_beat_family: String,
    /// Power-transition beat family (content-depth round 11): a beat keyed not to
    /// a stat or a time but to a *political* change — the first tick the dominant
    /// faction differs from the one the skeleton last marked (demographic drift
    /// grew a minority into the majority, or a schism unseated the largest people),
    /// a beat is forced from this family: the ship reckoning with new leadership.
    /// Empty = no power-transition beat.
    #[serde(default)]
    pub power_transition_beat_family: String,
    /// Anniversary cadence (content-depth round 7): every this-many years of the
    /// voyage, a beat is forced from `anniversary_beat_family` — a periodic
    /// archetype (vs the threshold beats), giving the voyage a commemorative
    /// heartbeat as the founding recedes into ritual over the centuries. 0 = off.
    #[serde(default)]
    pub anniversary_years: u32,
    /// The family an anniversary beat draws from.
    #[serde(default)]
    pub anniversary_beat_family: String,
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
            events: Self::load_events()?,
            legacies: DataRegistry::from_embedded_json(LEGACIES_JSON, "id")?,
            contracts: DataRegistry::from_embedded_json(CONTRACTS_JSON, "id")?,
            factions: DataRegistry::from_embedded_json(FACTIONS_JSON, "id")?,
            subsystems: DataRegistry::from_embedded_json(SUBSYSTEMS_JSON, "id")?,
            dynasty_names: load_embedded_json_labeled("dynasty_names", DYNASTY_NAMES_JSON)?,
            crew_archetypes: load_embedded_json_labeled("crew_archetypes", CREW_ARCHETYPES_JSON)?,
            texture_manifest: load_embedded_json(TEXTURE_MANIFEST_JSON)?,
        })
    }

    /// Merge the per-family event files into one registry. Fails loudly on a
    /// duplicate id *across* files — a single file makes a collision obvious, but
    /// two files can each define the same id and `merge` would silently drop one.
    fn load_events() -> Result<DataRegistry<EventTemplate>, String> {
        let mut merged: DataRegistry<EventTemplate> = DataRegistry::new();
        for (family, json) in EVENT_FILES {
            let part = DataRegistry::<EventTemplate>::from_embedded_json(json, "id")
                .map_err(|e| format!("events/{family}.json: {e}"))?;
            for id in part.ids() {
                if merged.contains(id) {
                    return Err(format!(
                        "duplicate event id '{id}' across event files (redefined in events/{family}.json)"
                    ));
                }
            }
            merged.merge(part);
        }
        Ok(merged)
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
    fn homecoming_lines_are_authored_for_every_success_level_and_substitute() {
        // Content-depth voice round 4: every mission outcome the game can log
        // must have homecoming prose, indexed deterministically and with the
        // voyage's length/generation woven in.
        let data = GameData::load().unwrap();
        let flavor = &data.config.flavor;
        for level in ["complete", "partial", "pyrrhic", "failure"] {
            let line = flavor
                .homecoming_line(level, 0, 450, 17)
                .unwrap_or_else(|| panic!("no homecoming prose for '{level}'"));
            assert!(
                line.contains("450") || line.contains("17"),
                "'{level}' homecoming should weave in the voyage's span: {line}"
            );
        }
        // Deterministic rotation by the index, and an unknown level is None.
        let a = flavor.homecoming_line("complete", 0, 300, 10);
        let b = flavor.homecoming_line("complete", 0, 300, 10);
        assert_eq!(a, b, "same index replays the same line");
        assert!(flavor.homecoming_line("triumphant", 0, 300, 10).is_none());
    }

    #[test]
    fn generational_turnover_voice_is_authored_and_varies() {
        // Content-depth voice round 5: the crew-retirement line fires several
        // times a generation, so its pool must have real variety (not a
        // repetition tell); the extinction ending must have authored prose too.
        let data = GameData::load().unwrap();
        let flavor = &data.config.flavor;
        assert!(
            flavor.retirement.len() >= 4,
            "the retirement pool needs variety — it fires several times a generation"
        );
        assert!(
            !flavor.extinction.is_empty(),
            "the line-ends ending needs prose"
        );
        // Consecutive retirements (the same generation) draw different lines.
        let a = FlavorConfig::line_with_name(&flavor.retirement, 0, "Vale").unwrap();
        let b = FlavorConfig::line_with_name(&flavor.retirement, 1, "Vale").unwrap();
        assert_ne!(
            a, b,
            "two stand-downs in one generation must read differently"
        );
        assert!(a.contains("Vale"), "the retiring holder's name is woven in");
    }

    #[test]
    fn crew_appointment_and_training_voice_is_authored_and_varies() {
        // Content-depth voice round 7: the appointment line (the positive twin of
        // retirement) and the training line both fire repeatedly as a roster is
        // re-crewed over the centuries, so both need pool variety and must weave
        // in the officer's name and human post — not the raw archetype id.
        let data = GameData::load().unwrap();
        let flavor = &data.config.flavor;
        assert!(
            flavor.appointment.len() >= 4,
            "the appointment pool needs variety — posts turn over all voyage"
        );
        assert!(
            flavor.training.len() >= 3,
            "the training pool needs variety — training is a repeatable verb"
        );
        // Two appointments in one drydock draw different lines, and both weave in
        // the officer's name and the human post name.
        let a = FlavorConfig::line_with_name_post(&flavor.appointment, 0, "Vale", "Chief Engineer")
            .unwrap();
        let b = FlavorConfig::line_with_name_post(&flavor.appointment, 1, "Vale", "Chief Engineer")
            .unwrap();
        assert_ne!(a, b, "two appointments must read differently");
        assert!(
            a.contains("Vale") && a.contains("Chief Engineer"),
            "the appointee's name and human post are woven in"
        );
        // The training pool carries the skill placeholder.
        assert!(
            flavor.training.iter().any(|s| s.contains("{skill}")),
            "training lines surface the new skill"
        );
    }

    #[test]
    fn faction_mood_voice_is_authored_and_names_the_people() {
        // Content-depth voice round 8: the approval meter's voice. A people
        // crossing into restlessness or contentment gets a pooled line, so both
        // pools need variety and must weave in the people's name.
        let data = GameData::load().unwrap();
        let flavor = &data.config.flavor;
        for (pool, label) in [
            (&flavor.faction_souring, "souring"),
            (&flavor.faction_warming, "warming"),
        ] {
            assert!(pool.len() >= 3, "the faction {label} pool needs variety");
            assert!(
                pool.iter().all(|s| s.contains("{name}")),
                "every faction {label} line must name the people"
            );
            let a = FlavorConfig::line_with_name(pool, 0, "the Keepers").unwrap();
            let b = FlavorConfig::line_with_name(pool, 1, "the Keepers").unwrap();
            assert_ne!(a, b, "consecutive {label} lines must differ");
            assert!(
                a.contains("the Keepers"),
                "the {label} line names the people"
            );
        }
    }

    #[test]
    fn ship_mood_voice_is_authored_and_varies() {
        // Content-depth voice round 11: the ship's collective morale crossing into
        // a grim or a buoyant band draws a pooled ambient line — the ship-wide twin
        // of the faction-mood voice. No name to weave, but both pools need variety.
        let data = GameData::load().unwrap();
        let flavor = &data.config.flavor;
        for (pool, label) in [
            (&flavor.ship_mood_darkening, "darkening"),
            (&flavor.ship_mood_lifting, "lifting"),
        ] {
            assert!(pool.len() >= 3, "the ship-mood {label} pool needs variety");
            let a = FlavorConfig::line_with_name(pool, 0, "").unwrap();
            let b = FlavorConfig::line_with_name(pool, 1, "").unwrap();
            assert_ne!(a, b, "consecutive ship-mood {label} lines must differ");
        }
    }

    #[test]
    fn subsystem_maintenance_voice_is_authored_and_names_the_module() {
        // Content-depth voice round 9: the field-repair and knowledge-training
        // verbs fire repeatedly across a voyage, so both pools need variety and
        // must weave in the module name.
        let data = GameData::load().unwrap();
        let flavor = &data.config.flavor;
        for (pool, label) in [
            (&flavor.subsystem_repair, "repair"),
            (&flavor.subsystem_training, "training"),
        ] {
            assert!(pool.len() >= 3, "the subsystem {label} pool needs variety");
            assert!(
                pool.iter().all(|s| s.contains("{name}")),
                "every subsystem {label} line must name the module"
            );
            let a = FlavorConfig::line_with_name(pool, 0, "engineering bay").unwrap();
            let b = FlavorConfig::line_with_name(pool, 1, "engineering bay").unwrap();
            assert_ne!(a, b, "consecutive {label} lines must differ");
            assert!(a.contains("engineering bay"), "the {label} line names it");
        }
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
                .chain(e.outcomes.iter().filter_map(|o| o.faction_loss_id.as_ref()))
                .chain(
                    e.outcomes
                        .iter()
                        .filter_map(|o| o.faction_merge_id.as_ref()),
                )
                // Content-depth round 6: complication faction gates too.
                .chain(
                    e.complications
                        .iter()
                        .map(|c| &c.requires_dominant_faction)
                        .filter(|f| !f.is_empty()),
                )
                .chain(
                    e.complications
                        .iter()
                        .flat_map(|c| c.requires_factions_aboard.iter()),
                )
                // Content-depth round 8: approval gate + approval-delta faction ids.
                .chain(e.faction_approval_below.iter().map(|g| &g.id))
                .chain(
                    e.outcomes
                        .iter()
                        .flat_map(|o| o.faction_approval_deltas.iter().map(|d| &d.id)),
                )
            {
                assert!(
                    data.factions.get(fid).is_some(),
                    "event '{id}' references unknown faction '{fid}'"
                );
            }
            // Content-depth subsystem↔event coupling: knowledge gates and
            // outcome subsystem deltas must name real subsystems.
            for sid in e
                .knowledge_below
                .iter()
                .map(|g| &g.id)
                .chain(e.condition_below.iter().map(|g| &g.id))
                .chain(
                    e.outcomes
                        .iter()
                        .flat_map(|o| o.subsystem_deltas.iter().map(|d| &d.id)),
                )
                // Content-depth round 12: outcome availability gates name
                // subsystems in their knowledge floors.
                .chain(
                    e.outcomes
                        .iter()
                        .flat_map(|o| o.requires.min_knowledge.iter().map(|f| &f.id)),
                )
                // Content-depth round 6: complication gates and deltas name
                // subsystems too.
                .chain(e.complications.iter().flat_map(|c| {
                    c.condition_below
                        .iter()
                        .map(|g| &g.id)
                        .chain(c.subsystem_deltas.iter().map(|d| &d.id))
                }))
            {
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
            for tag in e
                .requires_consequence
                .iter()
                .chain(
                    e.complications
                        .iter()
                        .flat_map(|c| c.requires_consequence.iter()),
                )
                // Content-depth round 12: outcome availability gates on a
                // consequence too.
                .chain(
                    e.outcomes
                        .iter()
                        .flat_map(|o| o.requires.requires_consequence.iter()),
                )
                // Content-depth round 13: the negative gate names consequences too.
                .chain(e.forbidden_consequence.iter())
            {
                assert!(
                    produced.contains(tag),
                    "event '{id}' gates on consequence '{tag}' no outcome records"
                );
            }
        }
        // Content-depth round 12: the first outcome of every event must be
        // unconditional, so a ship is never left with no legal choice and the
        // auto-resolve/index-0 contract always lands on an available outcome.
        for (id, e) in data.events.iter() {
            if let Some(first) = e.outcomes.first() {
                assert!(
                    first.requires.is_unconditional(),
                    "event '{id}' outcome 0 must be unconditional (gated outcomes come after)"
                );
            }
        }
        // Content-depth round 9: every scheduled follow-up must name a real event
        // (typo guard), and that target should be `scheduled_only` so the timed
        // payoff never also leaks into the reactive pool.
        for (id, e) in data.events.iter() {
            for followup in e
                .outcomes
                .iter()
                .filter_map(|o| o.schedule_followup.as_ref())
            {
                let target = data.events.get(&followup.template_id);
                assert!(
                    target.is_some(),
                    "event '{id}' schedules unknown follow-up '{}'",
                    followup.template_id
                );
                assert!(
                    target.unwrap().scheduled_only,
                    "scheduled follow-up '{}' must be marked scheduled_only",
                    followup.template_id
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
            // Content-depth faction coverage: every faction has at least one
            // signature event that fires while it runs the ship, so no group is
            // mechanically silent when dominant.
            assert!(
                data.events
                    .iter()
                    .any(|(_, e)| e.requires_dominant_faction == *id),
                "faction '{id}' has no signature (requires_dominant_faction) event"
            );
            // Content-depth round 7: every people brings a distinct recruitment
            // dowry — a personality, not a bare head count — and any subsystem it
            // lifts must be real.
            let boon = &faction.recruit_boon;
            assert!(
                !boon.flavor.trim().is_empty(),
                "faction '{id}' has no recruit_boon flavor"
            );
            for delta in &boon.subsystem_deltas {
                assert!(
                    data.subsystems.get(&delta.id).is_some(),
                    "faction '{id}' recruit_boon names unknown subsystem '{}'",
                    delta.id
                );
            }
            // Content-depth subsystems round 8: the module a people answers for
            // (its neglect erodes their approval) must be a real subsystem.
            assert!(
                faction.tended_subsystem.is_empty()
                    || data.subsystems.get(&faction.tended_subsystem).is_some(),
                "faction '{id}' tends unknown subsystem '{}'",
                faction.tended_subsystem
            );
            // Content-depth factions round 11: demographic drift is a gentle
            // per-generation share shift, not a population weapon.
            assert!(
                (-0.2..=0.2).contains(&faction.growth_bias),
                "faction '{id}' growth_bias {} out of the gentle range [-0.2, 0.2]",
                faction.growth_bias
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
                // Content-depth subsystems round 5: every tier carries its own
                // upgrade prose, so a rebuild never falls back to the generic
                // shared line.
                assert!(
                    !tier.flavor.trim().is_empty(),
                    "subsystem '{id}' has a tier with no upgrade flavor"
                );
            }
            // Content-depth subsystem coverage: every subsystem has at least one
            // knowledge-crisis event, so a module's know-how decaying always has
            // a beat to surface.
            assert!(
                data.events
                    .iter()
                    .any(|(_, e)| e.knowledge_below.iter().any(|g| &g.id == id)),
                "subsystem '{id}' has no knowledge_below crisis event"
            );
            // Content-depth subsystem coverage (round 4): and at least one
            // condition-breakdown event, so a module physically rotting always
            // has a beat to surface — the parallel to the knowledge crisis above.
            assert!(
                data.events
                    .iter()
                    .any(|(_, e)| e.condition_below.iter().any(|g| &g.id == id)),
                "subsystem '{id}' has no condition_below breakdown event"
            );
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
            let legacy = data.legacies.get(legacy_id).unwrap();
            assert!(
                legacy.dilemmas.len() >= 8,
                "{legacy_id} should have >= 8 dilemmas, has {}",
                legacy.dilemmas.len()
            );
            // Content-depth factions round 10: a dilemma option's faction-odds
            // modifier must name a real faction.
            for dil in &legacy.dilemmas {
                for opt in &dil.options {
                    assert!(
                        opt.dominant_faction.is_empty()
                            || data.factions.get(&opt.dominant_faction).is_some(),
                        "dilemma '{}' option '{}' names unknown faction '{}'",
                        dil.id,
                        opt.id,
                        opt.dominant_faction
                    );
                }
            }
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
            .chain(&sk.mid_pool)
            .chain(&sk.late_pool)
            .chain(&sk.dead_air_pool)
        {
            assert!(
                families.contains(fam),
                "campaign_skeleton pool family '{fam}' has no events"
            );
        }
        // Content-depth charters round 7: a charter's beat-pool bias must name
        // real families, or a biased beat could land on an empty pool. At least
        // one charter must carry a bias, so the mechanic stays exercised.
        assert!(
            data.contracts
                .iter()
                .any(|(_, c)| !c.beat_families.is_empty()),
            "some charter should bias its seeded skeleton via beat_families"
        );
        for (id, c) in data.contracts.iter() {
            for fam in &c.beat_families {
                assert!(
                    families.contains(fam),
                    "charter '{id}' beat_families '{fam}' has no events"
                );
            }
            // Content-depth charters round 9: a scripted timed beat must name a
            // real, scheduled_only event, and the beats must ascend by year so
            // they fire in order.
            for beat in &c.scheduled_beats {
                let target = data.events.get(&beat.template_id);
                assert!(
                    target.is_some_and(|e| e.scheduled_only),
                    "charter '{id}' scheduled beat '{}' must be a scheduled_only event",
                    beat.template_id
                );
            }
            assert!(
                c.scheduled_beats
                    .windows(2)
                    .all(|w| w[0].at_year <= w[1].at_year),
                "charter '{id}' scheduled_beats must ascend by at_year"
            );
            // Content-depth charters round 11: route hazard is a sane weight bump.
            assert!(
                (0.0..=1.0).contains(&c.hazard),
                "charter '{id}' hazard {} out of range [0, 1]",
                c.hazard
            );
            // Content-depth charters round 12: an in-world availability gate must
            // name real founding peoples, or the writ could never be offered.
            for fid in &c.requires_faction_aboard {
                assert!(
                    data.factions.get(fid).is_some(),
                    "charter '{id}' requires unknown faction '{fid}' aboard"
                );
            }
            // Content-depth charters round 13: a route toll must be a gentle,
            // survivable headwind — a per-year crew drain that could empty a
            // generational voyage is a bug, not a hazard.
            assert!(
                c.annual_toll.population.count.abs() <= 3,
                "charter '{id}' annual_toll drains {} crew/yr — too steep for a voyage",
                c.annual_toll.population.count
            );
        }
        // Content-depth charters round 13: at least one charter should carry a
        // standing route toll, so the mechanic is exercised.
        assert!(
            data.contracts.iter().any(|(_, c)| !c.annual_toll.is_none()),
            "some charter should exact a per-year route toll"
        );
        // Content-depth charters round 12: at least one charter should key on an
        // in-world gate, so the mechanic is exercised.
        assert!(
            data.contracts
                .iter()
                .any(|(_, c)| !c.requires_faction_aboard.is_empty()),
            "some charter should gate on a people being aboard"
        );
        // Content-depth round 5: the dead-air backstop needs a pool to draw from
        // when it is switched on, or a forced beat has nothing to force.
        if sk.dead_air_years > 0 {
            assert!(
                !sk.dead_air_pool.is_empty(),
                "dead_air_years is set but dead_air_pool is empty"
            );
        }
        // Content-depth threshold beats: each family they draw from must have
        // events, and thresholds must be ascending in (0, 1] so each fires once
        // in order. Same rules for drift (round 2) and adaptation (round 3).
        for (beats, family, label) in [
            (&sk.drift_beats, &sk.drift_beat_family, "drift"),
            (
                &sk.adaptation_beats,
                &sk.adaptation_beat_family,
                "adaptation",
            ),
        ] {
            if beats.is_empty() {
                continue;
            }
            assert!(
                families.contains(family),
                "campaign_skeleton {label}_beat_family '{family}' has no events"
            );
            assert!(
                beats.windows(2).all(|w| w[0] < w[1]),
                "campaign_skeleton {label}_beats must be strictly ascending"
            );
            assert!(
                beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton {label}_beats must be within (0, 1]"
            );
        }
        // Content-depth round 6: crisis beats are the DESCENDING mirror — the
        // ship's cohesion falling past each level in turn — so the same rules
        // hold but the thresholds must be strictly descending.
        if !sk.crisis_beats.is_empty() {
            assert!(
                families.contains(&sk.crisis_beat_family),
                "campaign_skeleton crisis_beat_family '{}' has no events",
                sk.crisis_beat_family
            );
            assert!(
                sk.crisis_beats.windows(2).all(|w| w[0] > w[1]),
                "campaign_skeleton crisis_beats must be strictly descending"
            );
            assert!(
                sk.crisis_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton crisis_beats must be within (0, 1]"
            );
            // Content-depth round 13: the recovery threshold must sit clear above
            // the highest crisis threshold, so a fractured ship must genuinely climb
            // out (a hysteresis band where neither beat fires) before it mends.
            if !sk.recovery_beat_family.is_empty() {
                let worst_crisis = sk.crisis_beats.iter().cloned().fold(0.0_f32, f32::max);
                assert!(
                    sk.recovery_beat_threshold > worst_crisis && sk.recovery_beat_threshold <= 1.0,
                    "recovery_beat_threshold {} must sit above the crisis band {worst_crisis}",
                    sk.recovery_beat_threshold
                );
            }
        }
        // Content-depth round 13: the recovery beat's family must have events.
        if !sk.recovery_beat_family.is_empty() {
            assert!(
                families.contains(&sk.recovery_beat_family),
                "campaign_skeleton recovery_beat_family '{}' has no events",
                sk.recovery_beat_family
            );
        }
        // Content-depth round 8: flourish beats are the ASCENDING positive pole —
        // morale climbing past each level in turn — so the thresholds must be
        // strictly ascending and in range, and the family must have events.
        if !sk.flourish_beats.is_empty() {
            assert!(
                families.contains(&sk.flourish_beat_family),
                "campaign_skeleton flourish_beat_family '{}' has no events",
                sk.flourish_beat_family
            );
            assert!(
                sk.flourish_beats.windows(2).all(|w| w[0] < w[1]),
                "campaign_skeleton flourish_beats must be strictly ascending"
            );
            assert!(
                sk.flourish_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton flourish_beats must be within [0, 1]"
            );
        }
        // Content-depth round 12: depopulation beats — founding-fraction thresholds
        // the crew falls past in turn, so strictly descending and in range, family
        // with events.
        if !sk.depopulation_beats.is_empty() {
            assert!(
                families.contains(&sk.depopulation_beat_family),
                "campaign_skeleton depopulation_beat_family '{}' has no events",
                sk.depopulation_beat_family
            );
            assert!(
                sk.depopulation_beats.windows(2).all(|w| w[0] > w[1]),
                "campaign_skeleton depopulation_beats must be strictly descending"
            );
            assert!(
                sk.depopulation_beats
                    .iter()
                    .all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton depopulation_beats must be within (0, 1]"
            );
        }
        // Content-depth round 9: objective-progress beats — mission-fraction
        // milestones, ascending and in range, family with events.
        if !sk.objective_beats.is_empty() {
            assert!(
                families.contains(&sk.objective_beat_family),
                "campaign_skeleton objective_beat_family '{}' has no events",
                sk.objective_beat_family
            );
            assert!(
                sk.objective_beats.windows(2).all(|w| w[0] < w[1]),
                "campaign_skeleton objective_beats must be strictly ascending"
            );
            assert!(
                sk.objective_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton objective_beats must be within [0, 1]"
            );
        }
        // Content-depth round 7: the periodic anniversary beat needs a family
        // with events when it is switched on.
        if sk.anniversary_years > 0 {
            assert!(
                families.contains(&sk.anniversary_beat_family),
                "campaign_skeleton anniversary_beat_family '{}' has no events",
                sk.anniversary_beat_family
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
        // Content-depth voice round 2: if ambient flavor is switched on, it needs
        // lines to draw from.
        if fl.ambient_gap_years > 0 {
            assert!(
                !fl.ambient.is_empty(),
                "ambient_gap_years is set but the ambient pool is empty"
            );
        }
        // Content-depth voice round 6: the recurring-crisis pools need variety
        // (they fire per year the crisis lasts), and famine weaves in its toll.
        assert!(
            fl.famine.len() >= 3 && fl.famine.iter().any(|s| s.contains("{losses}")),
            "the famine pool needs variety and a {{losses}} line"
        );
        assert!(
            fl.fuel_stall.len() >= 3,
            "the fuel-stall pool needs variety"
        );
        // Content-depth voice round 3: phase-line pool keys must be real phases.
        for key in fl.phase_lines.keys() {
            assert!(
                matches!(
                    key.as_str(),
                    "preparation" | "travel" | "operation" | "return" | "completion"
                ),
                "flavor.phase_lines has an unknown phase key '{key}'"
            );
        }
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
