//! Crew archetypes and dynasty name/trait pools (GDD §6).

use crate::data::ProductionRates;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewArchetype {
    pub id: String,
    pub name: String,
    pub description: String,
    pub skill_min: u32,
    pub skill_max: u32,
    /// Fractional production bonus per skill point per resource (e.g. food
    /// 0.005 at skill 60 → +30% food). Zeroed fields grant nothing.
    #[serde(default)]
    pub production_per_skill: ProductionRates,
    /// Fraction of famine losses prevented per skill point (medic).
    #[serde(default)]
    pub famine_loss_reduction_per_skill: f32,
    /// Yearly unity recovery per skill point while unity is depressed
    /// (security chief).
    #[serde(default)]
    pub unity_recovery_per_skill: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynastyNamePools {
    pub given_names: Vec<String>,
    pub surnames_by_legacy: HashMap<String, Vec<String>>,
    pub specializations: Vec<String>,
    pub traits_by_legacy: HashMap<String, Vec<String>>,
}
