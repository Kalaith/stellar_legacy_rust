//! Hull, drive and drydock tuning: what a ship costs to buy, to fit, to
//! repair, and what a voyage does to the people riding inside it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    /// How much the crew's *morale* swings objective accrual (content-depth charters
    /// round 22): the mission's coupling to the crew's spirits. Accrual is scaled by
    /// `1 + this·(morale − 0.5)`, floored — so a high-hearted crew drives the work
    /// faster and a dispirited one drags, around a neutral 0.5 midpoint. 0 = the crew's
    /// mood does not touch how fast the mission goes.
    #[serde(default)]
    pub morale_objective_swing: f32,
    /// How much the crew's *unity* swings objective accrual (content-depth charters round 34): the
    /// second crew-state accrual lever the round-22 morale coupling invited, and a distinct one — a
    /// crew can be high-hearted yet *fractured* (two contented peoples pulling different ways), or
    /// grim yet *united* (a dour crew rowing as one). Where morale is the work's *will*, unity is
    /// its *coordination*: a cohesive crew works a mission as a single hand while a divided one
    /// duplicates effort and argues the method. Accrual is scaled by `1 + this·(unity − 0.5)`,
    /// floored, around the neutral 0.5 midpoint, multiplying with the morale factor so a mission
    /// goes fastest under a crew both willing and united. 0 = the crew's cohesion does not touch how
    /// fast the mission goes.
    #[serde(default)]
    pub unity_objective_swing: f32,
    /// Success-chance bonus per point of aggregate combat on Wanderer dilemmas
    /// (firepower backs the confrontation).
    pub combat_dilemma_odds_per_point: f32,
    /// Ceiling on an effective dilemma success chance after the combat bonus.
    pub dilemma_odds_cap: f32,
    /// How much each point of aggregate *combat* dampens a charter's route `hazard` in the
    /// crisis-weight roll (content-depth charters round 27): a well-armed ship makes a lawless
    /// route think twice, so its guns cut into the route's own danger — the direct-firepower
    /// twin of `security_crisis_mitigation` (which quiets *every* crisis by the corps' internal
    /// order, where this deters only the *route's* added hazard). The deterred hazard is floored
    /// at 0, so firepower can neutralize a route's risk but never drop crises below the ship's
    /// base rate. 0 = the ship's guns do not deter the route (hazard reads as authored).
    #[serde(default)]
    pub hazard_combat_mitigation: f32,
    /// How much each point of aggregate *crew_capacity* (berths) eases a preserve charter's
    /// monthly attrition (content-depth charters round 28): crew_capacity's first mechanical
    /// role — until now a pure display stat — and the berth twin of cargo's haul lever. A ship
    /// with the room to carry its charge (colonists, refugees, the frozen) in some comfort loses
    /// fewer of them over the voyage; the attrition is scaled by `1 - crew·this`, floored so even
    /// the roomiest hull cannot wholly stop the loss. 0 = berths do not touch preserve attrition
    /// (a preserve charter erodes at its authored rate regardless of the ship).
    #[serde(default)]
    pub preserve_berth_relief: f32,
    /// How much a concluded mission's *outcome* moves the crew's morale (content-depth charters
    /// round 31): the crew's emotional stake in the ship's purpose. A mission seen through lifts
    /// spirits, one botched or abandoned dents them — `this·(score − 0.5)`, applied once at
    /// conclusion. Distinct from the pay, the reputation, and the faction goodwill an outcome
    /// earns; it composes with the round-29 despair / round-30 heartening beats. 0 = a mission's
    /// success or failure leaves the crew's spirits untouched.
    #[serde(default)]
    pub mission_outcome_morale_scale: f32,
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
    /// How much a well-kept medical bay resists the crew's *physiological* adaptation to
    /// the ship (content-depth subsystems round 25): the bodily twin of
    /// `archive_drift_resistance`. Where the archive's *knowledge* keeps the people
    /// *culturally* human (r10), the infirmary's *knowledge* — its living medical craft,
    /// the monitoring and gene-work a real generation ship's clinic would run — keeps
    /// them *physically* baseline, slowing the shipborn drift: `adaptation_per_year`
    /// scales by `1 - medical_adaptation_resistance * medical_bay_knowledge`. So a ship
    /// bound for a world can hold its crew fit to live on one, and a neglected infirmary
    /// lets the bodies go shipborn. 0 = the infirmary doesn't touch adaptation.
    #[serde(default)]
    pub medical_adaptation_resistance: f32,
    /// How much a *living biosphere* slows the shipborn adaptation (content-depth subsystems
    /// round 29): the environmental twin of `medical_adaptation_resistance`. Where the infirmary
    /// resists the drift by *knowledge* (the craft of managing the body), a lush agriculture
    /// resists it by *condition* — real food grown and eaten, green decks walked among, keep a
    /// crew a little more the kind of creature that could live on a world; `adaptation_per_year`
    /// scales by `1 - agriculture_adaptation_resistance * agriculture_condition`, stacking
    /// multiplicatively with the medical resistance. 0 = the biosphere doesn't touch adaptation.
    #[serde(default)]
    pub agriculture_adaptation_resistance: f32,
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
