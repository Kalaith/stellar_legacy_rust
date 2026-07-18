//! Save slots and migration (GDD §7): local persistence only, no server.

use crate::data::GameConfig;
use crate::state::sim::SimState;
use macroquad_toolkit::persistence::{
    load_from_slot_with_migration, save_to_slot_with_version, slot_exists,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub version: String,
    pub sim: SimState,
}

pub fn save_exists(config: &GameConfig) -> bool {
    slot_exists(&config.game_name, &config.save_slot)
}

pub fn save_campaign(config: &GameConfig, sim: &SimState) -> Result<(), String> {
    let save = SaveData {
        version: config.version.clone(),
        sim: sim.clone(),
    };
    save_to_slot_with_version(&config.game_name, &config.save_slot, &save, &config.version)
}

pub fn load_campaign(config: &GameConfig) -> Result<SimState, String> {
    let loaded: SaveData = load_from_slot_with_migration(
        &config.game_name,
        &config.save_slot,
        &config.version,
        |version, value| migrate_save_value(version, value, config),
    )?;
    Ok(loaded.sim)
}

/// v0.1.0 is the first shape; older/foreign payloads are rejected. When the
/// save format changes, add a real migration arm here instead of bumping the
/// version silently (the toolkit hands us the detected version).
pub fn migrate_save_value(
    detected_version: Option<String>,
    value: Value,
    config: &GameConfig,
) -> Result<SaveData, String> {
    let payload = value.get("data").cloned().unwrap_or(value);
    match serde_json::from_value::<SaveData>(payload) {
        Ok(mut save) => {
            save.version = config.version.clone();
            Ok(save)
        }
        Err(err) => Err(format!(
            "Unsupported save format {detected_version:?}: {err}"
        )),
    }
}
