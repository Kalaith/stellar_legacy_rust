//! Data-layer tests, split by the content area each one guards.
//!
//! The derivations more than one area needs live here as shared fixtures.

use std::collections::HashSet;

use super::*;

mod charters;
mod content;
mod economy;
mod event_gates;
mod parts;
mod peoples;
mod ship_systems;
mod skeleton;
mod voice;

/// Every event family that has authored events behind it. A beat or a bias that
/// names a family outside this set would draw from an empty pool.
fn authored_families(data: &GameData) -> HashSet<&String> {
    data.events.iter().map(|(_, e)| &e.family).collect()
}

/// Every reputation trait that something in the content actually nudges: an
/// event outcome, a charter's completion reward, or its abandonment penalty. A
/// gate naming a trait outside this set could never be met.
fn reputation_traits_produced(data: &GameData) -> HashSet<&String> {
    data.events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .flat_map(|o| o.reputation_deltas.iter().map(|r| &r.id))
        .chain(
            data.contracts
                .iter()
                .flat_map(|(_, c)| c.completion_reward.reputation_deltas.iter().map(|r| &r.id)),
        )
        .chain(
            data.contracts
                .iter()
                .flat_map(|(_, c)| c.abandonment.reputation_deltas.iter().map(|r| &r.id)),
        )
        .collect()
}
