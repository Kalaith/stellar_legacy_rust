//! Ship subsystem catalog (W5): the six module families beyond hull/engine/
//! weapon, each buffering one event family through upgrade tiers. Identities and
//! balance live in `assets/subsystems.json`; no subsystem constants in Rust.

use crate::data::ResourceDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemTier {
    /// Drydock cost to reach this tier from the one below (authored positive;
    /// negated on spend).
    pub cost: ResourceDelta,
    /// 0-1: fraction of a matching event's negative deltas prevented at full
    /// condition and this tier.
    pub severity_reduction: f32,
    /// Multiplier on a matching event's roll weight (0.8 = 20% rarer).
    pub weight_multiplier: f32,
    /// In-universe log line when a drydock upgrade reaches this tier (content-
    /// depth subsystems round 5: tier-specific flavor, replacing one generic
    /// "rebuilt stronger" line shared by all 6 modules × 3 tiers). Empty falls
    /// back to the built-in line so the log is never blank.
    #[serde(default)]
    pub flavor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDef {
    pub id: String,
    pub name: String,
    /// Event family this subsystem buffers (matches `EventTemplate.family`, W6).
    /// Empty means it buffers no family — it acts only through its extra effect.
    pub buffers_family: String,
    pub decay_per_year: f32,
    /// Institutional knowledge (0-1) needed to repair it.
    pub repair_knowledge_required: f32,
    pub repair_parts_cost: i64,
    pub repair_minerals_cost: i64,
    /// Tier 0 is the ship's baseline (no entry here); `tiers[i]` is the upgrade
    /// to reach tier `i + 1`.
    pub tiers: Vec<SubsystemTier>,
    pub description: String,
}

impl SubsystemDef {
    /// The active tier's stats, or `None` at baseline tier 0 (no buffering).
    pub fn tier_stats(&self, tier: u32) -> Option<&SubsystemTier> {
        if tier == 0 {
            None
        } else {
            self.tiers.get(tier as usize - 1)
        }
    }
}

/// Subsystem tunables (W5). All balance lives here, never in Rust.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SubsystemsConfig {
    pub knowledge_start: f32,
    pub knowledge_decay_per_generation: f32,
    pub education_transmission_per_tier: f32,
    pub train_knowledge_gain: f32,
    pub train_cost_credits: i64,
    pub agriculture_food_bonus_per_tier: f32,
}
