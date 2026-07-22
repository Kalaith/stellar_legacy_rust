//! Contract (mission) objective templates (GDD §5.2, §6).

use crate::data::{PopulationDelta, ResourceDelta, ShipDelta};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractObjective {
    Mining,
    Colonization,
    Exploration,
    Rescue,
    /// A residency among a living people, graded in standing/accords (content-depth
    /// charters round 8). An embassy is not a rescue — this gives the charter card
    /// the right word for a mission whose "yield" is a relationship.
    Diplomacy,
    /// Recovering mass from a wreck or derelict (content-depth charters round 8):
    /// a salvage haul is not mining — no seam is worked, a dead ship is stripped.
    Salvage,
}

impl ContractObjective {
    pub fn label(self) -> &'static str {
        match self {
            ContractObjective::Mining => "Mining",
            ContractObjective::Colonization => "Colonization",
            ContractObjective::Exploration => "Exploration",
            ContractObjective::Rescue => "Rescue",
            ContractObjective::Diplomacy => "Diplomacy",
            ContractObjective::Salvage => "Salvage",
        }
    }
}

/// preparation -> travel -> operation -> return -> completion (GDD §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractPhase {
    Preparation,
    Travel,
    Operation,
    Return,
    Completion,
}

impl ContractPhase {
    pub fn label(self) -> &'static str {
        match self {
            ContractPhase::Preparation => "Preparation",
            ContractPhase::Travel => "Travel",
            ContractPhase::Operation => "Operation",
            ContractPhase::Return => "Return",
            ContractPhase::Completion => "Completion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    PopulationSurvival,
    MissionCompletion,
    ResourceEfficiency,
    SocialCohesion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDef {
    pub id: String,
    pub kind: MetricKind,
    pub name: String,
    /// Weights across a template's metrics should sum to 1.0.
    pub weight: f32,
    pub target: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneDef {
    pub id: String,
    pub name: String,
    /// 0-1 progress fraction at which this milestone is reached.
    pub progress_threshold: f32,
    /// One-time resources granted when this milestone is first reached
    /// (PLAN item 3 — progress pays off along the way).
    #[serde(default)]
    pub reward: ResourceDelta,
}

/// One authored segment of a charter's fixed timeline (W2). The charter, never
/// the player, sets phase lengths; the years across a charter's segments sum
/// exactly to its `target_duration_years`. Only Travel / Operation / Return are
/// valid here — Preparation and Completion are the pre-launch and post-return
/// bookends, not authored segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseDef {
    pub kind: ContractPhase,
    pub years: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTemplate {
    pub id: String,
    pub name: String,
    pub objective: ContractObjective,
    pub description: String,
    pub target_duration_years: u32,
    /// Authored travel → operation → return timeline (W2). Sums to
    /// `target_duration_years`.
    pub phases: Vec<PhaseDef>,
    /// Quantified objective amount the mission must reach for full pay (W2):
    /// mine X, land X, chart X. Accrued only during Operation; pay is strictly
    /// proportional to the fraction of this reached.
    pub objective_target: f32,
    /// Human unit for the objective counter ("proof-of-yield cores", "settlers
    /// landed", "systems charted", "souls recovered").
    pub objective_unit: String,
    pub milestones: Vec<MilestoneDef>,
    pub success_metrics: Vec<MetricDef>,
    #[serde(default)]
    pub failure_risks: Vec<String>,
    #[serde(default)]
    pub reward: ResourceDelta,
    /// Chronicle renown a dynasty must have accrued before this charter unlocks
    /// (PLAN M4.8). 0 = available from the founding; richer charters gate higher.
    #[serde(default)]
    pub min_renown: i64,
    /// Free-form destination/mission tags (content-depth iteration) copied onto
    /// the active contract at launch. Events may gate on them via
    /// `EventTemplate::requires_charter_tag`, so a charter colors which events
    /// its voyage can surface (e.g. `hostile_space`, `garden`, `long_haul`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Beat-pool bias (content-depth charters round 7): extra event families
    /// layered into every seeded beat's draw for this voyage, so the charter
    /// shapes the campaign it generates. Must be real families with events.
    /// Empty = the standard phase/era pools only.
    #[serde(default)]
    pub beat_families: Vec<String>,
    /// Scripted timed beats (content-depth charters round 9): specific events this
    /// charter forces at determined voyage years, so a mission can be built around
    /// a reckoning on a *known clock* — a predicted stellar event, a treaty
    /// deadline. Each names a `scheduled_only` event; `at_year` is years since
    /// launch and the list must ascend. Empty = no scripted beats.
    #[serde(default)]
    pub scheduled_beats: Vec<ScheduledBeat>,
    /// Route hazard (content-depth charters round 11): the charter's *risk
    /// profile*, added to the immediate-crisis category weight for the whole
    /// voyage — a lawless derelict field or a star's reach breeds more crises than
    /// a quiet survey, so a dangerous writ *feels* dangerous (and pays for it in
    /// its reward). 0 = an ordinary route.
    #[serde(default)]
    pub hazard: f32,
    /// In-world availability gate (content-depth charters round 12): a writ offered
    /// only while these founding peoples are aboard *right now* — the charter-level
    /// parallel to the outcome gates, and the in-world twin of `min_renown`'s
    /// cross-campaign fame. So a mission a people is uniquely trusted with or called
    /// to appears only on a ship that carries them, and vanishes if they leave.
    /// Empty = offered to any ship (subject to renown).
    #[serde(default)]
    pub requires_faction_aboard: Vec<String>,
    /// Per-year toll the route exacts for its whole duration (content-depth charters
    /// round 13): hazard's deterministic companion. Where `hazard` breeds more
    /// crises (a stochastic danger), this is a steady drain applied every year of
    /// the voyage, so a route whose *nature* wears at a ship — a star's radiation
    /// bath, the grim haunt of a ship of the dead — feels that way continuously
    /// rather than only in the crises it throws. Default (all-zero) = no toll.
    #[serde(default)]
    pub annual_toll: AnnualToll,
    /// In-world availability gate on the ship's *deeds* (content-depth charters
    /// round 14): the writ is offered only once every listed consequence is on
    /// record — how a **charter arc** is built, so completing one mission unlocks
    /// the follow-on. The consequence-twin of `requires_faction_aboard`. Empty =
    /// no deed required.
    #[serde(default)]
    pub requires_consequence: Vec<String>,
    /// The negative twin (content-depth charters round 14): the writ is barred if
    /// *any* listed consequence is on record — delicate work a ship's dark history
    /// disqualifies it from. Empty = nothing bars it.
    #[serde(default)]
    pub forbidden_consequence: Vec<String>,
    /// Consequence recorded when this charter is seen through to full term
    /// (content-depth charters round 14): the seed of a **charter arc** — a survey
    /// completed proves the ground a later colony writ needs. Empty = the charter
    /// leaves no such mark.
    #[serde(default)]
    pub completion_consequence: String,
    /// The subsystem this mission's work leans on (content-depth subsystems round
    /// 14): the module whose condition scales how fast the objective accrues
    /// on-station — a mining survey's engineering bay, a greening's agriculture, a
    /// science dive's education archive. A well-kept module works the mission faster,
    /// a rotting one slower, so the subsystem axis at last touches the objective the
    /// voyage exists for. Empty = the objective is indifferent to any module's state.
    #[serde(default)]
    pub objective_subsystem: String,
    /// A lasting boon granted when this charter is seen through to full term
    /// (content-depth charters round 15): the mission's *legacy*, distinct from its
    /// pro-rated pay. Building a great extraction works makes the ship's engineers
    /// permanently better; greening a world deepens its gardeners' craft. Applied in
    /// the conclude path once, alongside the completion mark. Default (empty) = the
    /// charter leaves nothing but its pay.
    #[serde(default)]
    pub completion_reward: CompletionReward,
}

/// The lasting capability a charter grants on completion (content-depth charters
/// round 15): chiefly subsystem boons — a skill the ship keeps across voyages —
/// with optional resource/population lifts and the line that narrates it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionReward {
    #[serde(default)]
    pub subsystem_deltas: Vec<crate::data::events::SubsystemDelta>,
    #[serde(default)]
    pub resource: ResourceDelta,
    #[serde(default)]
    pub population: PopulationDelta,
    /// Line narrating the boon; empty = a generic line.
    #[serde(default)]
    pub log: String,
}

impl CompletionReward {
    /// True when the reward grants nothing (an ordinary charter with no legacy).
    pub fn is_none(&self) -> bool {
        self.subsystem_deltas.is_empty()
            && self.resource == ResourceDelta::default()
            && self.population == PopulationDelta::default()
    }
}

/// A charter's standing per-year toll (content-depth charters round 13). Each delta
/// is applied once per voyage-year while the contract is under way.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnualToll {
    #[serde(default)]
    pub resource: ResourceDelta,
    #[serde(default)]
    pub ship: ShipDelta,
    #[serde(default)]
    pub population: PopulationDelta,
}

impl AnnualToll {
    /// True when the toll is entirely zero (an ordinary route with no standing cost).
    pub fn is_none(&self) -> bool {
        self.resource == ResourceDelta::default()
            && self.ship == ShipDelta::default()
            && self.population == PopulationDelta::default()
    }
}

/// One scripted, time-fixed beat of a charter (content-depth charters round 9):
/// the named event is forced by id once the voyage has run `at_year` years.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledBeat {
    pub template_id: String,
    pub at_year: u32,
}
