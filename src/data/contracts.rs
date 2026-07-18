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
    /// Phase for a 0-1 progress fraction. Preparation is only the pre-launch
    /// state (progress 0 with no years elapsed is handled by the caller).
    pub fn for_progress(progress: f32) -> Self {
        if progress >= 1.0 {
            ContractPhase::Completion
        } else if progress >= 0.8 {
            ContractPhase::Return
        } else if progress >= 0.2 {
            ContractPhase::Operation
        } else {
            ContractPhase::Travel
        }
    }

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTemplate {
    pub id: String,
    pub name: String,
    pub objective: ContractObjective,
    pub description: String,
    pub target_duration_years: u32,
    pub milestones: Vec<MilestoneDef>,
    pub success_metrics: Vec<MetricDef>,
    #[serde(default)]
    pub failure_risks: Vec<String>,
    #[serde(default)]
    pub reward: ResourceDelta,
}
