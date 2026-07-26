//! Subsystem tests, split by what the module under test is being asked to do.
//! The two shared fixtures live here.

use super::*;
use crate::state::sim::founding_faction_ids;

mod effects;
mod life_support;
mod verbs;

fn campaign(seed: u64) -> (GameData, SimState) {
    let data = GameData::load().unwrap();
    let picks = founding_faction_ids(&data);
    let sim = SimState::new_campaign(&data, "preservers", seed, &picks);
    (data, sim)
}

fn data_swing() -> f32 {
    GameData::load()
        .unwrap()
        .config
        .subsystems
        .engineering_decay_swing
}
