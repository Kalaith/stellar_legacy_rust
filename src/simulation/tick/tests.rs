//! Tests for the advance loop and the economic year — split out of `tick.rs`
//! to keep it under the size limit.

use super::economy::factors::{
    apply_voyage_drift, energy_production_factor, influence_governance_factor, quiet_ambient_pool,
};
use super::*;
use crate::data::GameData;
use crate::simulation::contract::start_contract;

/// Content-depth factions round 21: strip every people's quiet-voice lines so the
/// *ordinary* ambient falls back to the generic pool. The condition-precedence
/// tests below predate the factions↔voice coupling and assert on the generic
/// ordinary lines — this keeps them testing precedence, not whoever runs the ship
/// (the coupling itself is covered by `the_ordinary_quiet_reads_in_the_dominant_peoples_voice`).
fn without_faction_voices(data: &mut GameData) {
    let ids: Vec<String> = data.factions.ids().cloned().collect();
    for id in ids {
        if let Some(mut f) = data.factions.remove(&id) {
            f.ambient.clear();
            data.factions.insert(id, f);
        }
    }
}

mod advance;
mod beats_collapse;
mod beats_recovery;
mod beats_scripted;
mod beats_threshold;
mod drift;
mod economy;
mod fuel;
mod succession;
mod voice;

fn fresh(seed: u64) -> (GameData, SimState) {
    let data = GameData::load().unwrap();
    let sim = SimState::new_campaign(
        &data,
        "preservers",
        seed,
        &crate::state::sim::founding_faction_ids(&data),
    );
    (data, sim)
}

fn provisioned(seed: u64, fuel: f32) -> (GameData, SimState) {
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", seed, &picks);
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    sim.resources.food = 10_000_000;
    sim.ship.fuel = fuel;
    (data, sim)
}
