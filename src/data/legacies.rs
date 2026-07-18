//! Legacy faction and dilemma definitions (GDD §5.5).

use crate::data::{PopulationDelta, ResourceDelta, ShipDelta};
use serde::{Deserialize, Serialize};

/// Effect applied on a dilemma option's success or failure branch. The
/// legacy-specific counters here update the *real* tracked state that the
/// original web build left as hardcoded placeholders (GDD §5.5 fix).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DilemmaEffect {
    pub resource_delta: ResourceDelta,
    pub ship_delta: ShipDelta,
    pub population_delta: PopulationDelta,
    pub tradition_points: i32,
    pub body_horror_events: u32,
    pub existential_dread: f32,
    pub piracy_reputation: f32,
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DilemmaOption {
    pub id: String,
    pub label: String,
    /// Probability the `success` branch applies; otherwise `failure` does.
    pub success_chance: f32,
    pub success: DilemmaEffect,
    pub failure: DilemmaEffect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DilemmaDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub options: Vec<DilemmaOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Failure-condition name: cultural_collapse / humanity_loss / fleet_dissolution.
    pub failure_risk: String,
    pub dilemmas: Vec<DilemmaDef>,
}
