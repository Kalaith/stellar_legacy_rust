//! Event template definitions (GDD §5.4, §6).

use crate::data::factions::FactionLossKind;
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
    /// When set, an applied outcome turns an active mission for home early (W2):
    /// the contract jumps to its Return segment. Fits both catastrophe (a crisis
    /// forcing withdrawal) and fortune (a find rich enough to sail back on).
    #[serde(default)]
    pub force_return: bool,
    /// When set, an applied outcome loses the ship's smallest faction this way
    /// (W7) — they settled off-ship or departed. Skipped if only one remains.
    #[serde(default)]
    pub faction_loss: Option<FactionLossKind>,
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
    /// Event family (W5, filled by W6). Matches a subsystem's `buffers_family`
    /// so the right module can soften or rarefy it. Empty = untagged.
    #[serde(default)]
    pub family: String,
    /// Contract phases this event may fire in (W6). Empty = any phase.
    #[serde(default)]
    pub phases: Vec<crate::data::contracts::ContractPhase>,
    /// Voyage gates (W6): the event stays out of the pool until the campaign has
    /// reached these. 0 / 0.0 = ungated.
    #[serde(default)]
    pub min_year: u32,
    #[serde(default)]
    pub min_generation: u32,
    #[serde(default)]
    pub min_cultural_drift: f32,
    /// Consequence chain gate (content-depth iteration): the event stays out of
    /// the pool until a prior outcome has recorded every tag listed here in
    /// `sim.consequences`. This is how an early choice re-fires a consequence
    /// decades later — the payoff half of `EventOutcome::long_term_consequences`.
    /// Empty = ungated.
    #[serde(default)]
    pub requires_consequence: Vec<String>,
    /// Multiplier on this template's roll weight per legacy id (GDD §6).
    #[serde(default)]
    pub legacy_weight_modifiers: HashMap<String, f32>,
    pub outcomes: Vec<EventOutcome>,
}
