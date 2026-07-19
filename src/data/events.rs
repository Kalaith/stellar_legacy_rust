//! Event template definitions (GDD §5.4, §6).

use crate::data::{PopulationDelta, ResourceDelta, ShipDelta};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    ImmediateCrisis,
    GenerationalChallenge,
    MissionMilestone,
    LegacyMoment,
}

impl EventCategory {
    pub const ALL: [EventCategory; 4] = [
        EventCategory::ImmediateCrisis,
        EventCategory::GenerationalChallenge,
        EventCategory::MissionMilestone,
        EventCategory::LegacyMoment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EventCategory::ImmediateCrisis => "Immediate Crisis",
            EventCategory::GenerationalChallenge => "Generational Challenge",
            EventCategory::MissionMilestone => "Mission Milestone",
            EventCategory::LegacyMoment => "Legacy Moment",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventOutcome {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub resource_delta: ResourceDelta,
    #[serde(default)]
    pub ship_delta: ShipDelta,
    #[serde(default)]
    pub population_delta: PopulationDelta,
    /// Named consequences recorded on the sim for later event weighting
    /// (Pillar 2: debts someone else pays). Each entry also costs 100 points
    /// in auto-resolve outcome scoring (GDD §5.4).
    #[serde(default)]
    pub long_term_consequences: Vec<String>,
    /// A ship component id this outcome drops into the salvage hold (PLAN M4.4).
    #[serde(default)]
    pub grant_component: Option<String>,
    #[serde(default)]
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTemplate {
    pub id: String,
    pub category: EventCategory,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub requires_decision: bool,
    /// Multiplier on this template's roll weight per legacy id (GDD §6).
    #[serde(default)]
    pub legacy_weight_modifiers: HashMap<String, f32>,
    pub outcomes: Vec<EventOutcome>,
}
