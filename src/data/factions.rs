//! Founding faction definitions (W7): authored population groups a campaign
//! can carry aboard. Identities live in `assets/factions.json`; no faction
//! names or balance numbers appear in Rust source.

use serde::{Deserialize, Serialize};

use crate::data::events::SubsystemDelta;
use crate::data::PopulationDelta;

/// What a people brings aboard when recruited in drydock (content-depth factions
/// round 7: recruitable pool personalities). A one-time signature boon so *which*
/// faction you take on matters beyond a head count — the makers bring their
/// craft, the gardeners their green thumb. All effects are data; empty = a bare
/// recruit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecruitBoon {
    /// Signature subsystem lift (condition/knowledge) their expertise grants.
    pub subsystem_deltas: Vec<SubsystemDelta>,
    /// Signature population lift (morale/unity/etc.) their arrival brings.
    pub population_delta: PopulationDelta,
    /// The line narrating what they bring; empty falls back to the generic one.
    pub flavor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    /// -1.0 (tech-averse) .. +1.0 (tech-embracing). Unused mechanically in v1;
    /// reserved for the modifiers a later pass will layer on.
    pub ideology: f32,
    pub description: String,
    /// Short phrase used in logs, e.g. "the Verdant Kin".
    pub log_name: String,
    /// The dowry this people brings when recruited (content-depth round 7).
    #[serde(default)]
    pub recruit_boon: RecruitBoon,
}

/// How a faction left the ship when an event drives it off (W7). WipedOut and
/// Assimilated arise from the simulation itself, not from an outcome, so they
/// are not represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionLossKind {
    Settled,
    Departed,
}

/// Faction tunables (W7). All balance lives here, never in Rust.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FactionConfig {
    /// How many factions a campaign founds with (the picker enforces it).
    pub starting_count: u32,
    /// Below this share of the people aboard, a faction may be assimilated…
    pub assimilation_share_threshold: f32,
    /// …but only once cultural drift has passed this.
    pub assimilation_drift_threshold: f32,
    pub recruit_group_cost_credits: i64,
    pub recruit_group_size: u32,
}
