//! Crew archetypes and dynasty name/trait pools (GDD §6).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewArchetype {
    pub id: String,
    pub name: String,
    pub description: String,
    pub skill_min: u32,
    pub skill_max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynastyNamePools {
    pub given_names: Vec<String>,
    pub surnames_by_legacy: HashMap<String, Vec<String>>,
    pub specializations: Vec<String>,
    pub traits_by_legacy: HashMap<String, Vec<String>>,
}
