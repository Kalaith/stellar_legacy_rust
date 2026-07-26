//! Charter tests, split by which part of a voyage's life they guard.
//! The two shared fixtures live here.

use super::*;
use crate::data::contracts::MetricKind;
use crate::data::GameData;

mod board;
mod completion;
mod progress;
mod scoring;

fn metric(weight: f32, target: f32, current: f32) -> MetricState {
    MetricState {
        id: "m".into(),
        kind: MetricKind::MissionCompletion,
        name: "m".into(),
        weight,
        target,
        current,
    }
}

fn armed(seed: u64, contract_id: &str) -> (crate::data::GameData, crate::state::sim::SimState) {
    let data = crate::data::GameData::load().unwrap();
    let mut sim = crate::state::sim::SimState::new_campaign(
        &data,
        "preservers",
        seed,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let template = data.contracts.get(contract_id).unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    (data, sim)
}
