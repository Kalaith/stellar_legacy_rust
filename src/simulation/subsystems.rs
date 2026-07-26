//! Ship-subsystem services (W5): yearly decay, generational knowledge transfer,
//! the repair/upgrade/train verbs, and the event buffering each module family
//! provides. All balance comes from data; this only reads ids and applies it.

use crate::data::subsystems::SubsystemDef;
use crate::data::{GameData, PopulationDelta, ResourceDelta, ShipDelta};
use crate::state::sim::subsystems::SubsystemState;
use crate::state::sim::SimState;

pub mod effects;
pub mod verbs;

pub use effects::*;
pub use verbs::*;

/// The catalog subsystem whose `buffers_family` matches `family`, in sorted-id
/// order (deterministic). `None` for the empty family or no match.
fn buffering_def<'a>(data: &'a GameData, family: &str) -> Option<&'a SubsystemDef> {
    if family.is_empty() {
        return None;
    }
    GameData::sorted_ids(&data.subsystems)
        .into_iter()
        .find_map(|id| {
            data.subsystems
                .get(&id)
                .filter(|d| d.buffers_family == family)
        })
}

/// Effective buffer strength (0-1) a subsystem provides right now: its current
/// tier's `severity_reduction` scaled by condition. Baseline tier 0 gives 0.
fn effective_severity(def: &SubsystemDef, state: &SubsystemState) -> f32 {
    match def.tier_stats(state.tier) {
        Some(tier) => (tier.severity_reduction * state.condition).clamp(0.0, 1.0),
        None => 0.0,
    }
}

/// Roll-weight factor for an event of `family` (W5): a buffering subsystem makes
/// its family rarer, scaled by condition — `1 - (1 - weight_multiplier) × cond`.
pub fn family_weight_factor(sim: &SimState, data: &GameData, family: &str) -> f32 {
    let Some(def) = buffering_def(data, family) else {
        return 1.0;
    };
    let Some(state) = sim.subsystems.get(&def.id) else {
        return 1.0;
    };
    let Some(tier) = def.tier_stats(state.tier) else {
        return 1.0;
    };
    1.0 - (1.0 - tier.weight_multiplier) * state.condition
}

/// Scale every NEGATIVE component of an outcome's deltas by the subsystem
/// buffering `family` (W5). Positive components are untouched. Returns the
/// buffered copies to apply.
pub fn buffered_deltas(
    sim: &SimState,
    data: &GameData,
    family: &str,
    resource: ResourceDelta,
    ship: ShipDelta,
    population: PopulationDelta,
) -> (ResourceDelta, ShipDelta, PopulationDelta) {
    let factor = match buffering_def(data, family) {
        Some(def) => match sim.subsystems.get(&def.id) {
            Some(state) => 1.0 - effective_severity(def, state),
            None => 1.0,
        },
        None => 1.0,
    };
    if factor >= 1.0 {
        return (resource, ship, population);
    }
    (
        scale_resource(resource, factor),
        scale_ship(ship, factor),
        scale_population(population, factor),
    )
}

fn soften_i64(x: i64, factor: f32) -> i64 {
    if x < 0 {
        (x as f32 * factor) as i64
    } else {
        x
    }
}

fn soften_i32(x: i32, factor: f32) -> i32 {
    if x < 0 {
        (x as f32 * factor) as i32
    } else {
        x
    }
}

fn soften_f32(x: f32, factor: f32) -> f32 {
    if x < 0.0 {
        x * factor
    } else {
        x
    }
}

fn scale_resource(d: ResourceDelta, f: f32) -> ResourceDelta {
    ResourceDelta {
        credits: soften_i64(d.credits, f),
        energy: soften_i64(d.energy, f),
        minerals: soften_i64(d.minerals, f),
        food: soften_i64(d.food, f),
        influence: soften_i64(d.influence, f),
    }
}

fn scale_ship(d: ShipDelta, f: f32) -> ShipDelta {
    ShipDelta {
        hull_integrity: soften_f32(d.hull_integrity, f),
        life_support: soften_f32(d.life_support, f),
        fuel: soften_f32(d.fuel, f),
        spare_parts: soften_i32(d.spare_parts, f),
    }
}

fn scale_population(d: PopulationDelta, f: f32) -> PopulationDelta {
    PopulationDelta {
        count: soften_i32(d.count, f),
        morale: soften_f32(d.morale, f),
        unity: soften_f32(d.unity, f),
        stability: soften_f32(d.stability, f),
        legacy_loyalty: soften_f32(d.legacy_loyalty, f),
        adaptation: soften_f32(d.adaptation, f),
        cultural_drift: soften_f32(d.cultural_drift, f),
    }
}

#[cfg(test)]
mod tests;
