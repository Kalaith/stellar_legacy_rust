//! Faction tests, split by what they cover. Shared fixtures live here.

use super::*;
use crate::data::factions::FactionLossKind;
use crate::data::GameData;
use crate::state::sim::founding_faction_ids;

mod announce;
mod condition;
mod recruit;
mod roster;
mod sentiment;
mod spillover;

fn fs(id: &str, members: u32) -> FactionState {
    FactionState {
        faction_id: id.to_owned(),
        members,
        status: FactionStatus::Aboard,
        approval: default_approval(),
        mood_band: 0,
    }
}

fn armed(seed: u64) -> (GameData, SimState, Vec<String>) {
    let data = GameData::load().unwrap();
    let picks = founding_faction_ids(&data);
    let sim = SimState::new_campaign(&data, "preservers", seed, &picks);
    (data, sim, picks)
}
