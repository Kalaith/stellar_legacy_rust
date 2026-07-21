//! Contract (mission) objective templates (GDD §5.2, §6).

use crate::data::ResourceDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractObjective {
    Mining,
    Colonization,
    Exploration,
    Rescue,
}

impl ContractObjective {
    pub fn label(self) -> &'static str {
        match self {
            ContractObjective::Mining => "Mining",
            ContractObjective::Colonization => "Colonization",
            ContractObjective::Exploration => "Exploration",
            ContractObjective::Rescue => "Rescue",
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
}
