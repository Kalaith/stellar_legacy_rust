//! The full serializable simulation state for one campaign.
//!
//! UI panels read this via `&SimState` and never mutate it directly — all
//! mutation happens through `UiAction` dispatch in `game.rs` and the
//! stateless services in `simulation/` (CODE_STANDARDS §7).

use crate::data::{GameData, PopulationDelta, ProductionRates, ResourceDelta, ShipDelta};
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricState {
    pub id: String,
    pub kind: crate::data::contracts::MetricKind,
    pub name: String,
    pub weight: f32,
    pub target: f32,
    pub current: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneState {
    pub id: String,
    pub name: String,
    pub progress_threshold: f32,
    pub reached: bool,
    /// One-time resources granted when first reached (PLAN item 3).
    #[serde(default)]
    pub reward: ResourceDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveContract {
    pub template_id: String,
    pub name: String,
    pub objective: crate::data::contracts::ContractObjective,
    pub target_duration_years: u32,
    /// Contract time elapsed, month-precise (W2/W3). Drives the phase timeline,
    /// the progress bar, and completion.
    pub months_elapsed: u32,
    /// Current phase, set from the authored segments — never derived from a
    /// fraction (W2).
    pub phase: crate::data::contracts::ContractPhase,
    /// The charter's authored travel → operation → return segments (W2), copied
    /// at start so the active contract carries its own timeline.
    pub phases: Vec<crate::data::contracts::PhaseDef>,
    /// Index into `phases` for the current segment.
    pub phase_index: usize,
    pub metrics: Vec<MetricState>,
    pub milestones: Vec<MilestoneState>,
    /// Population when the contract began, for the survival metric.
    pub starting_population: u32,
    /// Quantified objective amount for full pay (W2), copied from the charter.
    pub objective_target: f32,
    /// Human unit for the objective counter.
    pub objective_unit: String,
    /// Objective amount reached so far — accrues only during Operation (W2).
    pub objective_progress: f32,
}

impl ActiveContract {
    /// Total contract length in months.
    pub fn total_months(&self) -> u32 {
        self.target_duration_years * 12
    }

    /// Timeline position as a 0-1 fraction (milestones + the UI bar).
    pub fn progress(&self) -> f32 {
        let total = self.total_months();
        if total == 0 {
            1.0
        } else {
            (self.months_elapsed as f32 / total as f32).min(1.0)
        }
    }

    /// Fraction of the quantified objective reached — the pay multiplier (W2).
    /// A target of 0 counts as fully met.
    pub fn objective_fraction(&self) -> f32 {
        if self.objective_target <= 0.0 {
            1.0
        } else {
            (self.objective_progress / self.objective_target).clamp(0.0, 1.0)
        }
    }

    /// Total months of Operation across the authored segments (the window in
    /// which the objective can be worked).
    pub fn operation_months(&self) -> u32 {
        self.phases
            .iter()
            .filter(|p| p.kind == crate::data::contracts::ContractPhase::Operation)
            .map(|p| p.years * 12)
            .sum()
    }

    /// The segment index and phase kind for a given month of contract time.
    /// Month 0 is pre-launch Preparation; past the last segment is Completion.
    pub fn phase_at(&self, months: u32) -> (usize, crate::data::contracts::ContractPhase) {
        use crate::data::contracts::ContractPhase;
        if months == 0 {
            return (0, ContractPhase::Preparation);
        }
        let mut cumulative = 0;
        for (i, segment) in self.phases.iter().enumerate() {
            cumulative += segment.years * 12;
            if months <= cumulative {
                return (i, segment.kind);
            }
        }
        (
            self.phases.len().saturating_sub(1),
            ContractPhase::Completion,
        )
    }

    /// Index of the first Return segment, if the charter has one.
    pub fn first_return_index(&self) -> Option<usize> {
        self.phases
            .iter()
            .position(|p| p.kind == crate::data::contracts::ContractPhase::Return)
    }

    /// Cumulative month at which segment `i` begins.
    pub fn segment_start(&self, i: usize) -> u32 {
        self.phases[..i].iter().map(|s| s.years * 12).sum()
    }
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
    pub market: MarketState,
    pub delegation: DelegationSettings,
    pub pending_event: Option<PendingEvent>,
    #[serde(default)]
    pub pending_dilemma: Option<PendingDilemma>,
    /// Accumulated named consequences from past outcomes (Pillar 2). Read by
    /// future event weighting; append-only from outcome application.
    pub consequences: Vec<String>,
    pub log: Vec<LogEntry>,
}

impl SimState {
    /// Build a fresh campaign for the chosen legacy. Deterministic for a
    /// given (data, legacy, seed) triple — all randomness flows through the
    /// stored seeded RNG (GDD §5.6).
    pub fn new_campaign(data: &GameData, legacy_id: &str, seed: u64) -> Self {
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
            market,
            delegation: DelegationSettings::default(),
            pending_event: None,
            pending_dilemma: None,
            consequences: Vec::new(),
            log: Vec::new(),
        };
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
        let a = SimState::new_campaign(&data, "preservers", 42);
        let b = SimState::new_campaign(&data, "preservers", 42);
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
        let sim = SimState::new_campaign(&data, "wanderers", 7);
        let json = serde_json::to_string(&sim).unwrap();
        let back: SimState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dynasty.members.len(), sim.dynasty.members.len());
        assert_eq!(back.legacy.legacy_id, "wanderers");
    }
}
