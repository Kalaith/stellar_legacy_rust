//! Ship-subsystem runtime state (W5): per-subsystem tier, condition, and the
//! institutional knowledge that gates its repair. Knowledge is a per-subsystem
//! aggregate carried by the population — not per-crew, not per-faction — and
//! dies with the people unless the education subsystem transmits it forward.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::data::GameData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemState {
    /// 0..=3 (0 is the ship's baseline; 1..=3 are drydock upgrades).
    pub tier: u32,
    /// 0-1 physical condition; decays yearly, restored by repair.
    pub condition: f32,
    /// 0-1 institutional knowledge for THIS subsystem.
    pub knowledge: f32,
}

/// One runtime entry per catalog subsystem: baseline tier 0, whole condition,
/// and the founding knowledge stock (W5).
pub fn build_founding_subsystems(data: &GameData) -> HashMap<String, SubsystemState> {
    let start = data.config.subsystems.knowledge_start;
    data.subsystems
        .ids()
        .map(|id| {
            (
                id.clone(),
                SubsystemState {
                    tier: 0,
                    condition: 1.0,
                    knowledge: start,
                },
            )
        })
        .collect()
}
