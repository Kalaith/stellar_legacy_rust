//! The three meters a voyage lives or dies by: what the ship carries,
//! what condition it is in, and the people riding inside it.

use crate::data::{PopulationDelta, ResourceDelta, ShipDelta};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResourcePool {
    pub credits: i64,
    pub energy: i64,
    pub minerals: i64,
    pub food: i64,
    pub influence: i64,
}

impl ResourcePool {
    pub fn from_delta(d: ResourceDelta) -> Self {
        let mut pool = Self::default();
        pool.apply(&d);
        pool
    }

    /// Apply a signed delta, clamping every resource at zero.
    pub fn apply(&mut self, d: &ResourceDelta) {
        self.credits = (self.credits + d.credits).max(0);
        self.energy = (self.energy + d.energy).max(0);
        self.minerals = (self.minerals + d.minerals).max(0);
        self.food = (self.food + d.food).max(0);
        self.influence = (self.influence + d.influence).max(0);
    }

    /// True when every negative component of `cost` can be paid in full.
    pub fn can_afford(&self, cost: &ResourceDelta) -> bool {
        self.credits + cost.credits.min(0) >= 0
            && self.energy + cost.energy.min(0) >= 0
            && self.minerals + cost.minerals.min(0) >= 0
            && self.food + cost.food.min(0) >= 0
            && self.influence + cost.influence.min(0) >= 0
    }
}

/// Ship condition (GDD §5.1) plus the installed component loadout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipState {
    pub hull_integrity: f32,
    pub life_support: f32,
    pub fuel: f32,
    pub spare_parts: i64,
    pub hull: String,
    pub engine: String,
    pub weapon: Option<String>,
    /// Components found on the voyage but not yet installed (PLAN M4.4).
    /// Field-installable underway only if crew + part allow; freely in port.
    #[serde(default)]
    pub salvage: Vec<String>,
    /// Subsystem *version* ids a mission has granted (the fitting equivalent of
    /// `salvage`). A mission-reward subsystem version can never be bought — it can
    /// be fitted in drydock only once its id is unlocked here.
    #[serde(default)]
    pub unlocked_fittings: Vec<String>,
}

impl ShipState {
    pub fn apply(&mut self, d: &ShipDelta) {
        self.hull_integrity = (self.hull_integrity + d.hull_integrity).clamp(0.0, 1.0);
        self.life_support = (self.life_support + d.life_support).clamp(0.0, 1.0);
        self.fuel = (self.fuel + d.fuel).clamp(0.0, 1.0);
        self.spare_parts = (self.spare_parts + d.spare_parts as i64).max(0);
    }
}

/// Colony-scale aggregate population stats (GDD §5.1). Fractions are 0-1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationState {
    pub count: u32,
    pub morale: f32,
    pub unity: f32,
    pub stability: f32,
    pub legacy_loyalty: f32,
    pub adaptation: f32,
    pub cultural_drift: f32,
}

impl PopulationState {
    pub fn apply(&mut self, d: &PopulationDelta) {
        self.count = (self.count as i64 + d.count as i64).max(0) as u32;
        self.morale = (self.morale + d.morale).clamp(0.0, 1.0);
        self.unity = (self.unity + d.unity).clamp(0.0, 1.0);
        self.stability = (self.stability + d.stability).clamp(0.0, 1.0);
        self.legacy_loyalty = (self.legacy_loyalty + d.legacy_loyalty).clamp(0.0, 1.0);
        self.adaptation = (self.adaptation + d.adaptation).clamp(0.0, 1.0);
        self.cultural_drift = (self.cultural_drift + d.cultural_drift).clamp(0.0, 1.0);
    }
}
