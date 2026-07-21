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

/// A signed change to one named subsystem's condition and/or institutional
/// knowledge (content-depth iteration). Lets an event outcome wound a module,
/// patch it, teach a skill forward, or bury it with the experts who die.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDelta {
    pub id: String,
    #[serde(default)]
    pub condition: f32,
    #[serde(default)]
    pub knowledge: f32,
}

/// A crisis gate keyed to how much a subsystem's know-how has decayed
/// (content-depth iteration): the event only fires while that subsystem's
/// institutional knowledge has fallen to or below `below` — "the last person
/// who understood the reactor is dying" beats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGate {
    pub id: String,
    pub below: f32,
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
    /// Signed changes to named subsystems (content-depth iteration): condition
    /// and/or knowledge, clamped to [0, 1]. This is the coupling that lets an
    /// engineering crisis actually damage the engineering bay, or a teaching
    /// succession restore its lost know-how.
    #[serde(default)]
    pub subsystem_deltas: Vec<SubsystemDelta>,
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
    /// Charter-tag gate (content-depth iteration): the event only enters the
    /// pool while an active contract carries every tag listed here (see
    /// `ContractTemplate::tags`). This lets a destination color its own event
    /// pool — hostile space breeds boarding scares, garden runs breed settlers.
    /// Empty = any charter (or none).
    #[serde(default)]
    pub requires_charter_tag: Vec<String>,
    /// Faction-colored gate (content-depth iteration): the event only fires
    /// while this faction is the largest aboard — its signature situations
    /// surface when it runs the ship. Empty = any dominant faction.
    #[serde(default)]
    pub requires_dominant_faction: String,
    /// Inter-faction friction gate: every faction id listed must still be
    /// aboard for the event to fire (e.g. a quarrel between two rival groups).
    /// Empty = no faction-presence requirement.
    #[serde(default)]
    pub requires_factions_aboard: Vec<String>,
    /// Knowledge-crisis gates (content-depth iteration): the event only fires
    /// while every listed subsystem's knowledge has decayed to or below its
    /// threshold. Empty = ungated.
    #[serde(default)]
    pub knowledge_below: Vec<KnowledgeGate>,
    /// Multiplier on this template's roll weight per legacy id (GDD §6).
    #[serde(default)]
    pub legacy_weight_modifiers: HashMap<String, f32>,
    pub outcomes: Vec<EventOutcome>,
}
