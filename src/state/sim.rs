//! The full serializable simulation state for one campaign.
//!
//! UI panels read this via `&SimState` and never mutate it directly — all
//! mutation happens through `UiAction` dispatch in `game.rs` and the
//! stateless services in `simulation/` (CODE_STANDARDS §7).

use crate::data::{GameData, PopulationDelta, ProductionRates, ResourceDelta, ShipDelta};
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod contract;
pub mod factions;
pub mod subsystems;

pub use contract::{ActiveContract, CampaignBeat, MetricState, MilestoneState};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResourcePool {
    pub credits: i64,
    pub energy: i64,
    pub minerals: i64,
    pub food: i64,
    pub influence: i64,
}

impl ResourcePool {
    pub fn from_delta(d: ResourceDelta) -> Self {
        let mut pool = Self::default();
        pool.apply(&d);
        pool
    }

    /// Apply a signed delta, clamping every resource at zero.
    pub fn apply(&mut self, d: &ResourceDelta) {
        self.credits = (self.credits + d.credits).max(0);
        self.energy = (self.energy + d.energy).max(0);
        self.minerals = (self.minerals + d.minerals).max(0);
        self.food = (self.food + d.food).max(0);
        self.influence = (self.influence + d.influence).max(0);
    }

    /// True when every negative component of `cost` can be paid in full.
    pub fn can_afford(&self, cost: &ResourceDelta) -> bool {
        self.credits + cost.credits.min(0) >= 0
            && self.energy + cost.energy.min(0) >= 0
            && self.minerals + cost.minerals.min(0) >= 0
            && self.food + cost.food.min(0) >= 0
            && self.influence + cost.influence.min(0) >= 0
    }
}

/// Ship condition (GDD §5.1) plus the installed component loadout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipState {
    pub hull_integrity: f32,
    pub life_support: f32,
    pub fuel: f32,
    pub spare_parts: i64,
    pub hull: String,
    pub engine: String,
    pub weapon: Option<String>,
    /// Components found on the voyage but not yet installed (PLAN M4.4).
    /// Field-installable underway only if crew + part allow; freely in port.
    #[serde(default)]
    pub salvage: Vec<String>,
}

impl ShipState {
    pub fn apply(&mut self, d: &ShipDelta) {
        self.hull_integrity = (self.hull_integrity + d.hull_integrity).clamp(0.0, 1.0);
        self.life_support = (self.life_support + d.life_support).clamp(0.0, 1.0);
        self.fuel = (self.fuel + d.fuel).clamp(0.0, 1.0);
        self.spare_parts = (self.spare_parts + d.spare_parts as i64).max(0);
    }
}

/// Colony-scale aggregate population stats (GDD §5.1). Fractions are 0-1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationState {
    pub count: u32,
    pub morale: f32,
    pub unity: f32,
    pub stability: f32,
    pub legacy_loyalty: f32,
    pub adaptation: f32,
    pub cultural_drift: f32,
}

impl PopulationState {
    pub fn apply(&mut self, d: &PopulationDelta) {
        self.count = (self.count as i64 + d.count as i64).max(0) as u32;
        self.morale = (self.morale + d.morale).clamp(0.0, 1.0);
        self.unity = (self.unity + d.unity).clamp(0.0, 1.0);
        self.stability = (self.stability + d.stability).clamp(0.0, 1.0);
        self.legacy_loyalty = (self.legacy_loyalty + d.legacy_loyalty).clamp(0.0, 1.0);
        self.adaptation = (self.adaptation + d.adaptation).clamp(0.0, 1.0);
        self.cultural_drift = (self.cultural_drift + d.cultural_drift).clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynastyMember {
    pub id: u32,
    pub name: String,
    pub age: u32,
    /// 0-100 leadership skill; drives heir selection (GDD §5.3).
    pub leadership: u32,
    pub specialization: String,
    pub trait_name: String,
    pub is_leader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dynasty {
    pub generation: u32,
    pub years_since_generation: u32,
    pub next_member_id: u32,
    pub members: Vec<DynastyMember>,
    /// Council-designated successor (GDD §4 Select Heir). Honored at the
    /// next succession if still living and age-eligible.
    #[serde(default)]
    pub designated_heir: Option<u32>,
    /// Set when a generation tick finds no leader and no eligible heir.
    pub extinct: bool,
}

impl Dynasty {
    pub fn leader(&self) -> Option<&DynastyMember> {
        self.members.iter().find(|m| m.is_leader)
    }
}

/// One serving officer holding a ship post (GDD §4 Recruit/Train). At most
/// one crew member per archetype post; posts fall vacant on retirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewMember {
    pub id: u32,
    pub name: String,
    pub archetype_id: String,
    pub age: u32,
    /// 0-100, capped by the archetype's skill_max.
    pub skill: u32,
}

/// Per-legacy tracked inputs to the failure-risk formula (GDD §5.5). These
/// were hardcoded placeholders in the original web build; here they are real
/// state updated by dilemmas and events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyTrack {
    pub legacy_id: String,
    pub tradition_points: i32,
    pub body_horror_events: u32,
    pub existential_dread: f32,
    pub piracy_reputation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeResource {
    Energy,
    Minerals,
    Food,
    Influence,
}

impl TradeResource {
    pub const ALL: [TradeResource; 4] = [
        TradeResource::Energy,
        TradeResource::Minerals,
        TradeResource::Food,
        TradeResource::Influence,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TradeResource::Energy => "Energy",
            TradeResource::Minerals => "Minerals",
            TradeResource::Food => "Food",
            TradeResource::Influence => "Influence",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEntry {
    pub resource: TradeResource,
    pub price: f32,
    /// Signed change applied by the most recent yearly drift.
    pub trend: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    pub entries: Vec<MarketEntry>,
}

/// Per-category advisor delegation (GDD §5.4): a delegated category's events
/// auto-resolve via outcome scoring instead of blocking on the player.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DelegationSettings {
    pub immediate_crisis: bool,
    pub generational_challenge: bool,
    pub mission_milestone: bool,
    pub legacy_moment: bool,
}

impl DelegationSettings {
    pub fn is_delegated(&self, category: crate::data::events::EventCategory) -> bool {
        use crate::data::events::EventCategory::*;
        match category {
            ImmediateCrisis => self.immediate_crisis,
            GenerationalChallenge => self.generational_challenge,
            MissionMilestone => self.mission_milestone,
            LegacyMoment => self.legacy_moment,
        }
    }

    pub fn toggle(&mut self, category: crate::data::events::EventCategory) {
        use crate::data::events::EventCategory::*;
        match category {
            ImmediateCrisis => self.immediate_crisis = !self.immediate_crisis,
            GenerationalChallenge => self.generational_challenge = !self.generational_challenge,
            MissionMilestone => self.mission_milestone = !self.mission_milestone,
            LegacyMoment => self.legacy_moment = !self.legacy_moment,
        }
    }
}

/// An event waiting for a council decision. Stores the template id, not a
/// copy — the UI and resolver look the template up in `GameData`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEvent {
    pub template_id: String,
    /// Months-since-founding when the event fired (W3 month clock).
    pub rolled_month_clock: u32,
}

/// A follow-up event promised to fire at a determined voyage year (content-depth
/// event families round 9): the deterministic-timing counterpart to the
/// opportunistic `requires_consequence` chains. Queued by an outcome's
/// `schedule_followup`, fired once the voyage reaches `fire_year`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub template_id: String,
    /// Voyage year (years since founding) at or after which the follow-up fires.
    pub fire_year: u32,
}

/// A legacy dilemma waiting for a council decision (GDD §5.5). Stores the
/// dilemma id; the definition lives on the sim's legacy in `GameData`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDilemma {
    pub dilemma_id: String,
    /// Months-since-founding when the dilemma fired (W3 month clock).
    pub rolled_month_clock: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub year: u32,
    /// Calendar month 1-12 the line was stamped in (W3 month clock).
    pub month: u32,
    pub text: String,
}

/// How far one Advance press fast-forwards (W3). The advance loop always steps
/// month by month; this only caps how many months a single press covers before
/// it hard-stops on the next decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedStep {
    OneMonth,
    #[default]
    OneYear,
    FiveYears,
    TenYears,
}

impl SpeedStep {
    pub const ALL: [SpeedStep; 4] = [
        SpeedStep::OneMonth,
        SpeedStep::OneYear,
        SpeedStep::FiveYears,
        SpeedStep::TenYears,
    ];

    /// Months a single Advance at this step covers (before any decision stop).
    pub fn months(self) -> u32 {
        match self {
            SpeedStep::OneMonth => 1,
            SpeedStep::OneYear => 12,
            SpeedStep::FiveYears => 60,
            SpeedStep::TenYears => 120,
        }
    }

    /// Short label for the speed-selector row.
    pub fn label(self) -> &'static str {
        match self {
            SpeedStep::OneMonth => "1mo",
            SpeedStep::OneYear => "1yr",
            SpeedStep::FiveYears => "5yr",
            SpeedStep::TenYears => "10yr",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimState {
    pub seed: u64,
    pub rng: SeededRng,
    /// Months since founding, starting at 0 (W3). Display year/month derive
    /// from it via `year()` / `month()`; the economic tick still applies on
    /// year boundaries.
    pub month_clock: u32,
    /// Month-clock reading when the last event fired, for the event-chance
    /// ramp (GDD §5.4, now month-resolution).
    pub last_event_month_clock: u32,
    /// How far one Advance press fast-forwards (W3 speed selector).
    #[serde(default)]
    pub speed: SpeedStep,
    pub resources: ResourcePool,
    pub production: ProductionRates,
    pub ship: ShipState,
    pub population: PopulationState,
    pub dynasty: Dynasty,
    #[serde(default)]
    pub crew: Vec<CrewMember>,
    #[serde(default)]
    pub next_crew_id: u32,
    pub legacy: LegacyTrack,
    pub contract: Option<ActiveContract>,
    /// A charter under consideration in port before launch (W4). Cleared when
    /// the mission launches; only ever set while `contract.is_none()`.
    #[serde(default)]
    pub selected_charter: Option<String>,
    /// Total Travel months spent coasting on a dry tank (W4) — calendar time
    /// that bought no progress toward the destination.
    #[serde(default)]
    pub stalled_months: u32,
    /// Set the moment a Travel month stalls for want of fuel; read (and reset)
    /// at the year boundary to double that year's systems decay (W4).
    #[serde(default)]
    pub fuel_stalled_this_year: bool,
    /// Set when the player dismisses the first-voyage checklist; it also stops
    /// showing once the Chronicle records a completed mission.
    #[serde(default)]
    pub tutorial_dismissed: bool,
    pub market: MarketState,
    pub delegation: DelegationSettings,
    pub pending_event: Option<PendingEvent>,
    #[serde(default)]
    pub pending_dilemma: Option<PendingDilemma>,
    /// Accumulated named consequences from past outcomes (Pillar 2). Read by
    /// future event weighting; append-only from outcome application.
    pub consequences: Vec<String>,
    /// Follow-ups promised to fire at a *determined* year (content-depth event
    /// families round 9): an outcome can schedule a specific event to re-fire in
    /// N years, so an authored arc pays off on a clock rather than waiting for the
    /// RNG to surface it. Deterministic; fired and removed by `fire_scheduled_beat`.
    #[serde(default)]
    pub scheduled_events: Vec<ScheduledEvent>,
    /// How many times each event template has fired this campaign (content-depth
    /// event families round 11): lets a recurring crisis *escalate* instead of
    /// merely repeating — a complication can gate on prior occurrences, so the
    /// third outbreak of the same plague reads as the ship's patience wearing
    /// through. Incremented as each event resolves.
    #[serde(default)]
    pub event_fire_counts: HashMap<String, u32>,
    /// The dominant faction last marked by the skeleton (content-depth campaign
    /// skeleton round 11): so that when demographic drift or a schism flips *which
    /// people runs the ship*, a power-transition beat can fire on the change. Empty
    /// until the first tick records the launch majority (no spurious beat at start).
    #[serde(default)]
    pub last_dominant_faction: String,
    /// The morale band the ship's collective mood last announced (content-depth
    /// voice round 11): so a crossing *into* grim or buoyant surfaces one ambient
    /// line — the ship-wide parallel to a faction's `mood_band`. 0 (steady) at
    /// launch; settling back to steady is silent but remembered.
    #[serde(default)]
    pub morale_band: i8,
    /// How many depopulation thresholds the skeleton has already marked
    /// (content-depth campaign-skeleton round 12): the crew-thinning beat fires
    /// once per authored fraction of the founding size across the whole campaign
    /// (not per contract), so a recruited-up ship between voyages does not re-mark
    /// a stage it already passed. 0 at launch.
    #[serde(default)]
    pub depopulation_beats_fired: u32,
    /// Founding factions carried aboard (W7). `sum(members of Aboard) ==
    /// population.count` after every `rebalance_factions`.
    #[serde(default)]
    pub factions: Vec<factions::FactionState>,
    /// Ship subsystems keyed by catalog id (W5): tier, condition, knowledge.
    #[serde(default)]
    pub subsystems: HashMap<String, subsystems::SubsystemState>,
    pub log: Vec<LogEntry>,
}

impl SimState {
    /// Build a fresh campaign for the chosen legacy and founding factions.
    /// Deterministic for a given (data, legacy, seed, faction set) — all
    /// randomness flows through the stored seeded RNG (GDD §5.6). The caller
    /// guarantees `faction_ids` holds exactly `config.factions.starting_count`
    /// entries (the picker / `founding_faction_ids` enforce it).
    pub fn new_campaign(
        data: &GameData,
        legacy_id: &str,
        seed: u64,
        faction_ids: &[String],
    ) -> Self {
        let config = &data.config;
        let mut rng = SeededRng::new(seed);
        let dynasty = founding_dynasty(data, legacy_id, &mut rng);

        let market = MarketState {
            entries: TradeResource::ALL
                .iter()
                .map(|&resource| MarketEntry {
                    resource,
                    price: base_price(resource),
                    trend: 0.0,
                })
                .collect(),
        };

        let mut sim = Self {
            seed,
            rng,
            month_clock: 0,
            last_event_month_clock: 0,
            speed: SpeedStep::default(),
            resources: ResourcePool::from_delta(config.starting_resources),
            production: config.base_production,
            ship: ShipState {
                hull_integrity: 1.0,
                life_support: 1.0,
                fuel: 1.0,
                spare_parts: config.starting_spare_parts,
                hull: "colony_barge".to_owned(),
                engine: "ion_drive".to_owned(),
                weapon: None,
                salvage: Vec::new(),
            },
            population: PopulationState {
                count: config.starting_population,
                morale: 0.7,
                unity: 0.7,
                stability: 0.7,
                legacy_loyalty: 0.6,
                adaptation: 0.3,
                cultural_drift: 0.1,
            },
            dynasty,
            crew: Vec::new(),
            next_crew_id: 0,
            legacy: LegacyTrack {
                legacy_id: legacy_id.to_owned(),
                tradition_points: 50,
                body_horror_events: 0,
                existential_dread: 0.0,
                piracy_reputation: 0.0,
            },
            contract: None,
            selected_charter: None,
            stalled_months: 0,
            fuel_stalled_this_year: false,
            tutorial_dismissed: false,
            market,
            delegation: DelegationSettings::default(),
            pending_event: None,
            pending_dilemma: None,
            consequences: Vec::new(),
            scheduled_events: Vec::new(),
            event_fire_counts: HashMap::new(),
            last_dominant_faction: String::new(),
            morale_band: 0,
            depopulation_beats_fired: 0,
            factions: factions::build_founding_factions(faction_ids, config.starting_population),
            subsystems: subsystems::build_founding_subsystems(data),
            log: Vec::new(),
        };
        // Record the launch morale's band so the ship's hopeful starting spirits
        // read as the baseline, not a "lift" the collective-mood voice announces
        // (content-depth voice round 11).
        sim.morale_band = factions::mood_band_for(sim.population.morale);
        // Founding senior staff fill the configured starting posts.
        for archetype_id in &config.crew.starting_posts {
            let age_span = config.crew.recruit_age_max - config.crew.recruit_age_min + 1;
            let age = config.crew.recruit_age_min + sim.rng.below(age_span as usize) as u32;
            if let Some(member) = generate_crew_member(
                data,
                legacy_id,
                archetype_id,
                age,
                &mut sim.rng,
                &mut sim.next_crew_id,
            ) {
                sim.crew.push(member);
            }
        }
        // Name the peoples who board together (W7).
        let names: Vec<String> = faction_ids
            .iter()
            .map(|id| factions::log_name(&data.factions, id))
            .collect();
        if !names.is_empty() {
            sim.push_log(format!(
                "{} board together for the voyage.",
                join_names(&names)
            ));
        }
        sim.push_log("The founding council convenes. The voyage begins with a choice of contract.");
        sim
    }

    /// Whole years since founding (W3). Time is stored in months; this is the
    /// display/arithmetic year the rest of the game reasons about.
    pub fn year(&self) -> u32 {
        self.month_clock / 12
    }

    /// Calendar month 1-12 for display (W3).
    pub fn month(&self) -> u32 {
        self.month_clock % 12 + 1
    }

    /// True while any council decision (event or dilemma) blocks the tick.
    pub fn has_pending_decision(&self) -> bool {
        self.pending_event.is_some() || self.pending_dilemma.is_some()
    }

    pub fn push_log(&mut self, text: impl Into<String>) {
        self.log.push(LogEntry {
            year: self.year(),
            month: self.month(),
            text: text.into(),
        });
    }

    pub fn trim_log(&mut self, limit: usize) {
        if self.log.len() > limit {
            let excess = self.log.len() - limit;
            self.log.drain(..excess);
        }
    }
}

pub fn base_price(resource: TradeResource) -> f32 {
    match resource {
        TradeResource::Energy => 2.0,
        TradeResource::Minerals => 5.0,
        TradeResource::Food => 3.0,
        TradeResource::Influence => 20.0,
    }
}

/// The default founding faction set (W7): the first `starting_count` faction
/// ids in sorted order. Used by the game's real entry point and by tests that
/// don't drive the picker. Reads only from data — no faction names in Rust.
pub fn founding_faction_ids(data: &GameData) -> Vec<String> {
    let mut ids = GameData::sorted_ids(&data.factions);
    ids.truncate(data.config.factions.starting_count as usize);
    ids
}

/// Comma-join names with a trailing "and" for the founding log line (W7).
fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// Generate the founding dynasty: one leader in their prime plus a spread of
/// relatives, named from the legacy's pools.
fn founding_dynasty(data: &GameData, legacy_id: &str, rng: &mut SeededRng) -> Dynasty {
    let mut dynasty = Dynasty {
        generation: 1,
        years_since_generation: 0,
        next_member_id: 0,
        members: Vec::new(),
        designated_heir: None,
        extinct: false,
    };

    let ages = [45u32, 38, 33, 22, 17];
    for (i, &age) in ages.iter().enumerate() {
        let mut member = generate_member(data, legacy_id, age, rng, &mut dynasty.next_member_id);
        member.is_leader = i == 0;
        dynasty.members.push(member);
    }
    dynasty
}

pub fn generate_member(
    data: &GameData,
    legacy_id: &str,
    age: u32,
    rng: &mut SeededRng,
    next_id: &mut u32,
) -> DynastyMember {
    let pools = &data.dynasty_names;
    let given = pick(&pools.given_names, rng);
    let surname = pools
        .surnames_by_legacy
        .get(legacy_id)
        .map(|names| pick(names, rng))
        .unwrap_or_else(|| "Voyager".to_owned());
    let specialization = pick(&pools.specializations, rng);
    let trait_name = pools
        .traits_by_legacy
        .get(legacy_id)
        .map(|traits| pick(traits, rng))
        .unwrap_or_default();

    let id = *next_id;
    *next_id += 1;

    DynastyMember {
        id,
        name: format!("{given} {surname}"),
        age,
        leadership: 30 + rng.below(51) as u32,
        specialization,
        trait_name,
        is_leader: false,
    }
}

/// Generate a named officer for a post, skill rolled within the archetype's
/// range. Returns None for an unknown archetype id.
pub fn generate_crew_member(
    data: &GameData,
    legacy_id: &str,
    archetype_id: &str,
    age: u32,
    rng: &mut SeededRng,
    next_id: &mut u32,
) -> Option<CrewMember> {
    let archetype = data.crew_archetypes.iter().find(|a| a.id == archetype_id)?;
    let pools = &data.dynasty_names;
    let given = pick(&pools.given_names, rng);
    let surname = pools
        .surnames_by_legacy
        .get(legacy_id)
        .map(|names| pick(names, rng))
        .unwrap_or_else(|| "Voyager".to_owned());
    let skill_span = (archetype.skill_max - archetype.skill_min + 1) as usize;
    let skill = archetype.skill_min + rng.below(skill_span) as u32;

    let id = *next_id;
    *next_id += 1;
    Some(CrewMember {
        id,
        name: format!("{given} {surname}"),
        archetype_id: archetype.id.clone(),
        age,
        skill,
    })
}

fn pick(pool: &[String], rng: &mut SeededRng) -> String {
    rng.choose(pool).cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn new_campaign_is_deterministic_for_same_seed() {
        let data = GameData::load().unwrap();
        let a = SimState::new_campaign(
            &data,
            "preservers",
            42,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let b = SimState::new_campaign(
            &data,
            "preservers",
            42,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let names_a: Vec<_> = a.dynasty.members.iter().map(|m| m.name.clone()).collect();
        let names_b: Vec<_> = b.dynasty.members.iter().map(|m| m.name.clone()).collect();
        assert_eq!(names_a, names_b);
        assert_eq!(a.dynasty.leader().unwrap().age, 45);
    }

    #[test]
    fn resource_pool_clamps_at_zero_and_checks_affordability() {
        let mut pool = ResourcePool {
            credits: 100,
            ..Default::default()
        };
        let cost = crate::data::ResourceDelta {
            credits: -150,
            ..Default::default()
        };
        assert!(!pool.can_afford(&cost));
        pool.apply(&cost);
        assert_eq!(pool.credits, 0);
    }

    #[test]
    fn sim_state_round_trips_through_serde() {
        let data = GameData::load().unwrap();
        let sim = SimState::new_campaign(
            &data,
            "wanderers",
            7,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let json = serde_json::to_string(&sim).unwrap();
        let back: SimState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dynasty.members.len(), sim.dynasty.members.len());
        assert_eq!(back.legacy.legacy_id, "wanderers");
    }
}
