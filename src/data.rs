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
    /// Food store at or above which a year counts as *fat* (content-depth
    /// provisioning round 14): the symmetric mirror of `lean_food_threshold` — the
    /// "comfortably flush" line whose sustained crossing drives `fat_food_years`, the
    /// state that separates a windfall year from a lifetime of plenty. 0 = disabled.
    #[serde(default)]
    pub fat_food_threshold: i64,
    /// Years of sustained lean the crew endures before chronic hunger begins to wear
    /// their spirits (content-depth provisioning round 17): the provisioning axis's
    /// first *systemic* coupling. Once `lean_food_years` reaches this, the year tick
    /// drains a little morale each year the lean holds — so a grinding multi-year
    /// hunger doesn't merely gate content (it89) and read hungry (voice r13), it
    /// mechanically wears the ship down. A single bad winter stays below it (the acute
    /// famine events' domain). 0 = no chronic-hunger toll.
    #[serde(default)]
    pub chronic_hunger_years: u32,
    /// Morale drained per year while the ship is in a sustained lean past
    /// `chronic_hunger_years` (content-depth provisioning round 17). Gentle by design —
    /// the slow attrition of a hunger that will not end, not a single hard blow.
    #[serde(default)]
    pub chronic_hunger_morale_drain: f32,
    /// Extra *monthly death chance* added to every character while the ship has been
    /// lean past `chronic_hunger_years` (content-depth provisioning round 18 — the
    /// provisioning axis's coupling to the real-time-loop mortality system). Where
    /// `chronic_hunger_morale_drain` wears the crew's *spirits*, this wears their
    /// *bodies*: a hunger that grinds on for years thins the roster, the old and weak
    /// first. Added to the age curve (a well-kept infirmary still eases it, the hard
    /// age cap still holds). Gentle — the slow toll of long want, not a famine's blow.
    /// 0 = chronic hunger costs no lives directly.
    #[serde(default)]
    pub chronic_hunger_death_bonus: f32,
    /// Fractional boost to the dynasty's yearly renewal while the ship has stood in
    /// sustained plenty past `chronic_hunger_years` (content-depth provisioning round
    /// 19 — the positive pole of `chronic_hunger_death_bonus`, and the mirror of the
    /// hunger's toll). Where a long lean thins the roster, a long plenty fills the
    /// cradles: a well-fed generation raises more of its young to their majority, so
    /// the birth chance is multiplied by `1 + this` while the fat years hold. A second
    /// lever (with the habitat, it152) on the renewal that stands between the line and
    /// extinction. 0 = plenty gives no renewal boost.
    #[serde(default)]
    pub sustained_plenty_birth_bonus: f32,
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
    /// Per-character aging + death (real-time loop follow-up).
    pub mortality: MortalityConfig,
    pub failure_risk: FailureRiskConfig,
    pub ship: ShipConfig,
    /// Per-year population drift over a voyage (PLAN M4.1).
    pub voyage_drift: VoyageDrift,
    /// Field-vs-port repair tunables (PLAN M4.3).
    pub repair: RepairConfig,
    /// Real-time voyage pacing (real-time loop): auto-advance cadence, decision
    /// auto-resolve timeout, and ranged-impact tuning.
    pub real_time: RealTimeConfig,
    /// Fixed campaign seed for reproducible testing (real-time loop follow-up).
    /// When set, every New Game uses this exact seed, so the same events fire in
    /// the same order run to run. `null` (the default) picks a fresh random seed
    /// per campaign from the wall-clock-seeded generator.
    #[serde(default)]
    pub fixed_seed: Option<u64>,
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

/// Real-time voyage pacing (real-time loop): while a mission is under way the
/// month clock auto-advances one month every `seconds_per_month` real seconds,
/// scaled by the 1×/2×/3× speed selector. A blocked council decision auto-
/// resolves to a random option after `decision_timeout_secs`. `impact_variance`
/// / `impact_min_magnitude_for_range` drive the ranged event impacts (a delta of
/// magnitude ≥ the minimum is shown as a band and rolled within it).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RealTimeConfig {
    pub seconds_per_month: f32,
    pub decision_timeout_secs: f32,
    pub impact_variance: f32,
    pub impact_min_magnitude_for_range: i64,
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
    /// A serving officer dies at their post (real-time loop follow-up: characters
    /// age and die on a monthly roll, not only at generation ticks). Placeholders
    /// `{name}`, `{post}`. Indexed by the officer's id; empty falls back.
    #[serde(default)]
    pub crew_death: Vec<String>,
    /// A starving year (content-depth voice round 6): fires once per *year* the
    /// larder is empty, so a multi-year famine needs variety or it reprints one
    /// line. Placeholder: `{losses}`. Indexed by year; empty falls back.
    #[serde(default)]
    pub famine: Vec<String>,
    /// A year coasting on a dry tank (content-depth voice round 6): like famine,
    /// fires once per stalled year. Indexed by year; empty falls back.
    #[serde(default)]
    pub fuel_stall: Vec<String>,
    /// The ramscoop/scanners replenishing reaction mass (real-time loop follow-up:
    /// legible stat changes): a periodic in-world report of the fuel the drive has
    /// gathered and processed over the last few travel years, so the fuel gauge's
    /// rise reads as *something the ship did* rather than an unexplained jump.
    /// Placeholder `{amount}` (whole fuel points gained). Indexed by year; empty =
    /// no fuel-replenishment narration.
    #[serde(default)]
    pub fuel_gain: Vec<String>,
    /// How many voyage years between provisioning reports (real-time loop follow-up):
    /// the fuel-gain line fires at most once per this many years, and only when a
    /// meaningful haul has actually accrued, so a long crossing gets an occasional
    /// legible "here is where your fuel comes from" beat without one line a year.
    /// 0 = disabled.
    #[serde(default)]
    pub fuel_report_gap_years: u32,
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
    /// The ship's *political climate* crossing into broad discontent (content-depth
    /// voice round 15): distinct from the crew's spirits — the peoples as a whole
    /// growing restive about their treatment. No name; indexed by year; empty =
    /// silence.
    #[serde(default)]
    pub polity_souring: Vec<String>,
    /// The political climate crossing into broad ease (content-depth voice round
    /// 15): the peoples as a whole settling, content with their lot. No name.
    #[serde(default)]
    pub polity_warming: Vec<String>,
    /// The ship crossing into a *merciful* reputation (content-depth voice round 16):
    /// the quiet marker, at a gentler threshold than the it109 beat, that the ship's
    /// name has begun to mean kindness in the dark. No name; indexed by year. Empty =
    /// silence. Watches the `campaign_skeleton.reputation_beat_trait`.
    #[serde(default)]
    pub reputation_merciful: Vec<String>,
    /// The ship crossing into a *feared* reputation (content-depth voice round 16):
    /// the mirror — its name beginning to mean the hard thing done without flinching.
    #[serde(default)]
    pub reputation_feared: Vec<String>,
    /// Reputation levels at/above which the ship remarks a merciful name (`_high`) or
    /// at/below which it remarks a feared one (`_low`) — gentler than the beat bands,
    /// so the voice precedes the reckoning (content-depth voice round 16).
    #[serde(default)]
    pub reputation_voice_high: f32,
    #[serde(default)]
    pub reputation_voice_low: f32,
    /// The ship's *institutions* crossing into disorder (content-depth voice round 17):
    /// the governance twin of the morale (`ship_mood_darkening`) and polity
    /// (`polity_souring`) voices — distinct from the crew's spirits and from how
    /// content the peoples are, this voices the *machinery of government* beginning to
    /// slip: quorums missed, offices going unfilled, decisions drifting. Gated at a
    /// gentler threshold than the it102 collapse *beat*, so the voice (a fraying
    /// noticed) precedes the reckoning (a government failed). No name; indexed by year;
    /// empty = silence.
    #[serde(default)]
    pub stability_fraying: Vec<String>,
    /// The ship's institutions crossing into good order (content-depth voice round 17):
    /// the positive twin — councils reaching quorum again, offices filled, the charter
    /// honored in practice, the government visibly working. No name; indexed by year.
    #[serde(default)]
    pub stability_firming: Vec<String>,
    /// Stability at/above which the ship remarks its institutions in good order
    /// (`_high`) or at/below which it remarks them fraying (`_low`) — the `_low`
    /// gentler than the it102 collapse-beat bands, so the voice precedes the reckoning
    /// (content-depth voice round 17).
    #[serde(default)]
    pub stability_voice_high: f32,
    #[serde(default)]
    pub stability_voice_low: f32,
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
    /// Ambient lines for a *long-prosperous* ship (content-depth voice round 14):
    /// the first positive-condition ambient — the mirror of `ambient_lean`. Once the
    /// larder has stood full for `ambient_fat_years_threshold` years
    /// (`SimState.fat_food_years`) *and* no grimmer note holds, the quiet stretches
    /// draw from this pool — the texture of ease and plenty, so a ship's good years
    /// finally *sound* good instead of merely neutral. Lowest priority (a grim ship
    /// reads grim first). Empty = a prosperous ship reads the ordinary ambient.
    #[serde(default)]
    pub ambient_fat: Vec<String>,
    /// Consecutive fat years at or past which quiet stretches read from
    /// `ambient_fat` (content-depth voice round 14).
    #[serde(default)]
    pub ambient_fat_years_threshold: u32,
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
    /// Loyalty-collapse thresholds (content-depth round 14): the last identity stat
    /// without a beat. As the people's `legacy_loyalty` falls to or below each
    /// threshold (authored high→low), a beat is forced — not the *cultural* drift the
    /// drift beats mark (becoming someone new) but the *political* one: the founders'
    /// covenant lapsing, a generation that no longer treats the founding charter as
    /// binding. Empty = no loyalty beats.
    #[serde(default)]
    pub loyalty_beats: Vec<f32>,
    /// The family a loyalty-collapse beat draws from.
    #[serde(default)]
    pub loyalty_beat_family: String,
    /// Governance-collapse thresholds (content-depth round 15): the last population
    /// stat without a beat. As `stability` falls to or below each threshold (high→
    /// low), a beat is forced — not the *people* fracturing (the crisis beat) nor
    /// the *founders'* authority lapsing (the loyalty beat), but the ship's own
    /// institutions ceasing to function: councils that cannot reach quorum, offices
    /// unfilled, the charter gone to folklore. Empty = no stability beats.
    #[serde(default)]
    pub stability_beats: Vec<f32>,
    /// The family a governance-collapse stability beat draws from.
    #[serde(default)]
    pub stability_beat_family: String,
    /// Reputation beat (content-depth round 16): the skeleton's first trigger on the
    /// ship's *cumulative character* (it105) rather than a population stat. When the
    /// named reputation trait crosses *into* a strong band — famously high (≥ `high`)
    /// or notoriously low (≤ `low`) — a beat is forced: the ship reckoning with the
    /// name it has earned. A return to the middle re-arms it. Empty trait/family = off.
    #[serde(default)]
    pub reputation_beat_trait: String,
    #[serde(default)]
    pub reputation_beat_high: f32,
    #[serde(default)]
    pub reputation_beat_low: f32,
    #[serde(default)]
    pub reputation_beat_family: String,
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
    /// Succession beat family (content-depth round 18 — the first beat keyed to the
    /// real-time-loop continuous-mortality system): the month a *sitting leader dies
    /// in office*, a beat is forced from this family — the ship reckoning with a
    /// captain lost mid-voyage and an untried heir in the chair. A planned retirement
    /// handoff does not fire it. Empty = no succession beat.
    #[serde(default)]
    pub succession_beat_family: String,
    /// Long-reign beat (content-depth campaign skeleton round 19 — the hopeful mirror
    /// of the succession beat): once a *sitting leader* has held the first chair for
    /// `long_reign_years`, a beat is forced from `long_reign_beat_family` — the ship
    /// reckoning with an era defined by one enduring hand, rare now that continuous
    /// mortality takes most leaders young. Fires once per reign (a succession re-arms
    /// it). 0 / empty = no long-reign beat.
    #[serde(default)]
    pub long_reign_years: u32,
    #[serde(default)]
    pub long_reign_beat_family: String,
    /// Anniversary cadence (content-depth round 7): every this-many years of the
    /// voyage, a beat is forced from `anniversary_beat_family` — a periodic
    /// archetype (vs the threshold beats), giving the voyage a commemorative
    /// heartbeat as the founding recedes into ritual over the centuries. 0 = off.
    #[serde(default)]
    pub anniversary_years: u32,
    /// The family an anniversary beat draws from.
    #[serde(default)]
    pub anniversary_beat_family: String,
    /// Subsystem-collapse beats (content-depth round 17): the first forced skeleton
    /// beat keyed to a *subsystem's condition* rather than a stat, time, phase, the
    /// objective, or a political change — the physical-crisis dimension the beat
    /// lattice never watched. The first tick a listed module's condition falls to or
    /// below its red line, a beat is forced from its family: the ship reckoning with
    /// a keystone that has *truly* failed, a guaranteed reckoning where before only a
    /// reactive condition-gated event might (or might not) roll. Campaign-scoped —
    /// fires once per module a voyage, tracked by id, so a repaired-then-re-collapsed
    /// module does not re-mark. Empty = no subsystem beats.
    #[serde(default)]
    pub subsystem_beats: Vec<SubsystemBeat>,
}

/// One subsystem-collapse beat (content-depth campaign skeleton round 17): when the
/// named module's `condition` first falls to or below `threshold`, a beat is forced
/// from `family` — the physical-crisis trigger the beat lattice lacked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemBeat {
    pub subsystem: String,
    pub threshold: f32,
    pub family: String,
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

/// Per-character mortality (real-time loop follow-up: characters age and die).
/// Aging is a shared "Founding Day" event — everyone gains a year on the last
/// day of the year, whatever their true birthdate — but *death* is a monthly
/// roll whose odds climb with age: a flat accident chance at any age, plus an
/// age-scaled term that switches on past `onset_age` and doubles every
/// `doubling_years`. Certain at `member_max_age`. A heavy population-loss event
/// can also claim a named crew officer or relative.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MortalityConfig {
    /// Age past which the age-scaled monthly death term switches on.
    pub onset_age: u32,
    /// Monthly death chance at `onset_age` (before the accident floor).
    pub monthly_base_chance: f32,
    /// Years over which the age-scaled term doubles.
    pub doubling_years: f32,
    /// Flat monthly death chance at any age (accidents, mishaps).
    pub monthly_accident_chance: f32,
    /// A population loss of at least this many souls in one outcome may also
    /// take a named character.
    pub event_death_loss_threshold: u32,
    /// Chance a qualifying population-loss event claims a named character.
    pub event_death_chance: f32,
    /// The dynasty size the line renews toward. Each Founding Day, while the
    /// dynasty sits below this and has at least two members to carry it on, new
    /// young adults come of age (see `annual_birth_chance`) — the counterweight to
    /// the death roll, so a healthy line churns individuals without dying out.
    pub dynasty_target_size: u32,
    /// Per open slot below `dynasty_target_size`, the yearly chance a new young
    /// adult comes of age. Higher fills a depleted line back up faster.
    pub annual_birth_chance: f32,
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
                // Content-depth round 8/19: approval gate (both poles) + delta ids.
                .chain(e.faction_approval_below.iter().map(|g| &g.id))
                .chain(e.faction_approval_above.iter().map(|g| &g.id))
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
        // Content-depth round 16: a reputation gate must name a trait some outcome
        // actually nudges, or the ship could never build past its neutral 0.5 to
        // meet it (typo guard).
        let rep_produced: std::collections::HashSet<&String> = data
            .events
            .iter()
            .flat_map(|(_, e)| e.outcomes.iter())
            .flat_map(|o| o.reputation_deltas.iter().map(|r| &r.id))
            // Content-depth round 17: a charter's completion also nudges reputation.
            .chain(
                data.contracts
                    .iter()
                    .flat_map(|(_, c)| c.completion_reward.reputation_deltas.iter().map(|r| &r.id)),
            )
            // Content-depth round 18: and its abandonment marks the ship's name too.
            .chain(
                data.contracts
                    .iter()
                    .flat_map(|(_, c)| c.abandonment.reputation_deltas.iter().map(|r| &r.id)),
            )
            .collect();
        for (id, e) in data.events.iter() {
            for gate in e
                .min_reputation
                .iter()
                .chain(e.max_reputation.iter())
                // Content-depth round 17: outcome availability gates on reputation too.
                .chain(e.outcomes.iter().flat_map(|o| {
                    o.requires
                        .min_reputation
                        .iter()
                        .chain(o.requires.max_reputation.iter())
                }))
            {
                assert!(
                    rep_produced.contains(&gate.id),
                    "event '{id}' gates on reputation '{}' no outcome nudges",
                    gate.id
                );
            }
        }
        // Content-depth charters round 16: charter reputation gates name a real trait too.
        for (id, c) in data.contracts.iter() {
            for gate in c.min_reputation.iter().chain(c.max_reputation.iter()) {
                assert!(
                    rep_produced.contains(&gate.id),
                    "charter '{id}' gates on reputation '{}' no outcome nudges",
                    gate.id
                );
            }
        }
        // Content-depth factions round 16: a dominant-faction reputation leaning must
        // name a real trait and be a gentle lean, not a lever.
        for (id, f) in data.factions.iter() {
            for (trait_id, lean) in &f.reputation_leanings {
                assert!(
                    rep_produced.contains(trait_id),
                    "faction '{id}' leans reputation '{trait_id}' no outcome nudges"
                );
                assert!(
                    (-1.0..=1.0).contains(lean),
                    "faction '{id}' reputation lean {lean} out of range [-1, 1]"
                );
            }
        }
        // Content-depth round 14: a complication that targets specific choices must
        // name real outcomes of its own event (typo guard), or the toll could never
        // land.
        for (id, e) in data.events.iter() {
            let outcome_ids: std::collections::HashSet<&String> =
                e.outcomes.iter().map(|o| &o.id).collect();
            for c in &e.complications {
                for oid in &c.applies_to_outcomes {
                    assert!(
                        outcome_ids.contains(oid),
                        "event '{id}' complication '{}' targets unknown outcome '{oid}'",
                        c.id
                    );
                }
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
            // Content-depth factions round 14: a rival must be a real, other people,
            // and rivalries must be authored *symmetric* (if A names B, B names A) —
            // a one-sided grudge is an authoring slip.
            for rival in &faction.rivals {
                assert_ne!(rival, id, "faction '{id}' lists itself as a rival");
                let other = data
                    .factions
                    .get(rival)
                    .unwrap_or_else(|| panic!("faction '{id}' names unknown rival '{rival}'"));
                assert!(
                    other.rivals.contains(id),
                    "rivalry '{id}' <-> '{rival}' is not symmetric"
                );
            }
            // Content-depth factions round 17: an ally must likewise be a real, other
            // people; alliances symmetric; and a pair is never both kin and rival —
            // the positive and negative spillover would fight over the same relation.
            for ally in &faction.allies {
                assert_ne!(ally, id, "faction '{id}' lists itself as an ally");
                let other = data
                    .factions
                    .get(ally)
                    .unwrap_or_else(|| panic!("faction '{id}' names unknown ally '{ally}'"));
                assert!(
                    other.allies.contains(id),
                    "alliance '{id}' <-> '{ally}' is not symmetric"
                );
                assert!(
                    !faction.rivals.contains(ally),
                    "'{id}' <-> '{ally}' is listed as both ally and rival"
                );
            }
        }
        // Content-depth factions round 14: at least one people should carry a rival,
        // so the spillover mechanic is exercised.
        assert!(
            data.factions.iter().any(|(_, f)| !f.rivals.is_empty()),
            "some faction should have a standing rival"
        );
        // Content-depth factions round 17: and at least one a standing ally, so the
        // positive spillover is exercised too.
        assert!(
            data.factions.iter().any(|(_, f)| !f.allies.is_empty()),
            "some faction should have a standing ally"
        );

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
            // Content-depth charters round 19: a completion goodwill reward must name
            // a real people, or the goodwill would land nowhere.
            for d in &c.completion_reward.faction_approval_deltas {
                assert!(
                    data.factions.get(&d.id).is_some(),
                    "charter '{id}' completion_reward names unknown faction '{}'",
                    d.id
                );
            }
            // Content-depth charters round 20: a completion component reward must name
            // a real ship component, or the salvage hold gains a phantom.
            if let Some(comp) = &c.completion_reward.grant_component {
                assert!(
                    data.ship_components.find_any(comp).is_some(),
                    "charter '{id}' completion_reward grant_component '{comp}' is not a real component"
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
            // Content-depth subsystems round 14: the module a mission leans on must
            // be a real subsystem, or its condition could never scale the work.
            assert!(
                c.objective_subsystem.is_empty()
                    || data.subsystems.get(&c.objective_subsystem).is_some(),
                "charter '{id}' objective_subsystem names unknown module '{}'",
                c.objective_subsystem
            );
            // Content-depth charters round 15: a completion reward's subsystem boons
            // must name real modules, or the legacy could never land.
            for delta in &c.completion_reward.subsystem_deltas {
                assert!(
                    data.subsystems.get(&delta.id).is_some(),
                    "charter '{id}' completion_reward names unknown module '{}'",
                    delta.id
                );
            }
        }
        // Content-depth charters round 13: at least one charter should carry a
        // standing route toll, so the mechanic is exercised.
        assert!(
            data.contracts.iter().any(|(_, c)| !c.annual_toll.is_none()),
            "some charter should exact a per-year route toll"
        );
        // Content-depth charters round 14: a charter's deed gates must name a
        // consequence *something* produces — an event outcome or another charter's
        // completion — or the writ (or its bar) could never resolve (typo guard).
        let charter_produced: std::collections::HashSet<&String> = data
            .events
            .iter()
            .flat_map(|(_, e)| e.outcomes.iter())
            .flat_map(|o| o.long_term_consequences.iter())
            .chain(
                data.contracts
                    .iter()
                    .filter(|(_, c)| !c.completion_consequence.is_empty())
                    .map(|(_, c)| &c.completion_consequence),
            )
            .collect();
        for (id, c) in data.contracts.iter() {
            for tag in c
                .requires_consequence
                .iter()
                .chain(c.forbidden_consequence.iter())
            {
                assert!(
                    charter_produced.contains(tag),
                    "charter '{id}' gates on consequence '{tag}' nothing records"
                );
            }
        }
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
        // Content-depth round 14: loyalty-collapse beats are the DESCENDING mirror
        // on legacy_loyalty — strictly descending, in range, family with events.
        if !sk.loyalty_beats.is_empty() {
            assert!(
                families.contains(&sk.loyalty_beat_family),
                "campaign_skeleton loyalty_beat_family '{}' has no events",
                sk.loyalty_beat_family
            );
            assert!(
                sk.loyalty_beats.windows(2).all(|w| w[0] > w[1]),
                "campaign_skeleton loyalty_beats must be strictly descending"
            );
            assert!(
                sk.loyalty_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton loyalty_beats must be within (0, 1]"
            );
        }
        // Content-depth round 16: the reputation beat's family must have events, its
        // trait must be one some outcome nudges, and its band thresholds must order.
        if !sk.reputation_beat_family.is_empty() {
            assert!(
                families.contains(&sk.reputation_beat_family),
                "campaign_skeleton reputation_beat_family '{}' has no events",
                sk.reputation_beat_family
            );
            assert!(
                data.events.iter().any(|(_, e)| e.outcomes.iter().any(|o| o
                    .reputation_deltas
                    .iter()
                    .any(|r| r.id == sk.reputation_beat_trait))),
                "campaign_skeleton reputation_beat_trait '{}' no outcome nudges",
                sk.reputation_beat_trait
            );
            assert!(
                sk.reputation_beat_low < sk.reputation_beat_high
                    && (0.0..=1.0).contains(&sk.reputation_beat_low)
                    && (0.0..=1.0).contains(&sk.reputation_beat_high),
                "campaign_skeleton reputation beat bands must order within [0, 1]"
            );
        }
        // Content-depth round 15: stability beats are the DESCENDING governance
        // mirror — strictly descending, in range, family with events.
        if !sk.stability_beats.is_empty() {
            assert!(
                families.contains(&sk.stability_beat_family),
                "campaign_skeleton stability_beat_family '{}' has no events",
                sk.stability_beat_family
            );
            assert!(
                sk.stability_beats.windows(2).all(|w| w[0] > w[1]),
                "campaign_skeleton stability_beats must be strictly descending"
            );
            assert!(
                sk.stability_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
                "campaign_skeleton stability_beats must be within (0, 1]"
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
        // Content-depth round 17: subsystem-collapse beats — each names a real
        // module, a red line in (0, 1], and a family with events.
        for beat in &sk.subsystem_beats {
            assert!(
                data.subsystems.get(&beat.subsystem).is_some(),
                "campaign_skeleton subsystem_beat names unknown module '{}'",
                beat.subsystem
            );
            assert!(
                beat.threshold > 0.0 && beat.threshold <= 1.0,
                "campaign_skeleton subsystem_beat '{}' threshold {} must be within (0, 1]",
                beat.subsystem,
                beat.threshold
            );
            assert!(
                families.contains(&beat.family),
                "campaign_skeleton subsystem_beat '{}' family '{}' has no events",
                beat.subsystem,
                beat.family
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
        // Content-depth round 18: the succession beat (a sitting leader dying in
        // office) needs a family with events when set.
        if !sk.succession_beat_family.is_empty() {
            assert!(
                families.contains(&sk.succession_beat_family),
                "campaign_skeleton succession_beat_family '{}' has no events",
                sk.succession_beat_family
            );
        }
        // Content-depth round 19: the long-reign beat needs a family with events
        // when switched on.
        if sk.long_reign_years > 0 && !sk.long_reign_beat_family.is_empty() {
            assert!(
                families.contains(&sk.long_reign_beat_family),
                "campaign_skeleton long_reign_beat_family '{}' has no events",
                sk.long_reign_beat_family
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
