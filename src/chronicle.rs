//! Cross-playthrough chronicle (GDD §7).
//!
//! Persists outside any save slot so it survives across playthroughs.
//! v1 scope: an honest completed-contract log. Heritage modifiers (small
//! bonuses for a new dynasty derived from past entries) are the next step —
//! see PLAN.md M2/M3.

use macroquad_toolkit::persistence::{
    load_from_slot_with_migration, save_to_slot_with_version, slot_exists,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub completed_year: u32,
    pub contract_name: String,
    pub objective: String,
    pub legacy_id: String,
    pub leader_name: String,
    pub generation: u32,
    pub score: f32,
    pub outcome: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChronicleStore {
    pub entries: Vec<ChronicleEntry>,
}

impl ChronicleStore {
    pub fn load(game_name: &str, slot: &str, version: &str) -> Self {
        if !slot_exists(game_name, slot) {
            return Self::default();
        }
        load_from_slot_with_migration(game_name, slot, version, |_, value| {
            serde_json::from_value(value.get("data").cloned().unwrap_or(value))
                .map_err(|err| format!("chronicle migration failed: {err}"))
        })
        .unwrap_or_default()
    }

    pub fn save(&self, game_name: &str, slot: &str, version: &str) -> Result<(), String> {
        save_to_slot_with_version(game_name, slot, self, version)
    }

    pub fn record(&mut self, entry: ChronicleEntry) {
        self.entries.push(entry);
    }
}
