//! Event rolling, outcome scoring, and resolution (GDD §5.4).

pub mod outcome;
pub mod rolling;
pub mod skeleton;

pub use outcome::*;
pub use rolling::*;

use crate::data::events::{Complication, EventOutcome, EventTemplate};
use crate::data::{GameData, RealTimeConfig};
use crate::simulation::subsystems;
use crate::state::sim::SimState;
use macroquad_toolkit::rng::SeededRng;

/// The one complication (content-depth round 6) riding this event right now, if
/// any: the first, in authored order, whose gates all hold for the current sim.
/// The sim is paused while an event blocks, so this returns the same answer at
/// present-time (to append its description) and apply-time (to land its deltas).
pub fn active_complication<'a>(
    sim: &SimState,
    template: &'a EventTemplate,
) -> Option<&'a Complication> {
    template.complications.iter().find(|c| {
        sim.population.cultural_drift >= c.min_cultural_drift
            // Adaptation-divergence gate (content-depth round 30): rides only on a shipborn crew.
            && c.adaptation_above
                .is_none_or(|t| sim.population.adaptation >= t)
            && c.condition_below.iter().all(|gate| {
                sim.subsystems
                    .get(&gate.id)
                    .is_some_and(|s| s.condition <= gate.below)
            })
            && c.requires_consequence
                .iter()
                .all(|tag| sim.consequences.contains(tag))
            && c.food_below.is_none_or(|t| sim.resources.food <= t)
            && (c.requires_dominant_faction.is_empty()
                || sim.dominant_faction_id() == Some(c.requires_dominant_faction.as_str()))
            && c.requires_factions_aboard
                .iter()
                .all(|id| sim.is_faction_aboard(id))
            // Recurrence escalation (content-depth round 11): rides only once this
            // same event has already fired at least this many times.
            && sim
                .event_fire_counts
                .get(&template.id)
                .copied()
                .unwrap_or(0)
                >= c.min_prior_occurrences
            // Lived-state gates (content-depth round 15): a thinned crew, a long hunger.
            && (c.max_population == 0 || sim.population.count <= c.max_population)
            && sim.lean_food_years >= c.min_lean_food_years
            // …and its abundance twin (round 23): a crew grown soft on a long plenty.
            && sim.fat_food_years >= c.min_fat_food_years
            // Reputation gates (content-depth round 22): the name the ship has earned.
            && c.min_reputation
                .iter()
                .all(|g| sim.reputation(&g.id) >= g.threshold)
            && c.max_reputation
                .iter()
                .all(|g| sim.reputation(&g.id) <= g.threshold)
    })
}

/// Whether an outcome should be offered to this ship right now (content-depth
/// event families round 12): true unless its availability gate names a past
/// consequence not on record or a subsystem whose knowledge is below the floor.
/// The sim is paused while an event blocks, so this answers identically at
/// present-time (the modal) and apply-time.
pub fn outcome_available(sim: &SimState, outcome: &EventOutcome) -> bool {
    if outcome.requires.is_unconditional() {
        return true;
    }
    outcome
        .requires
        .requires_consequence
        .iter()
        .all(|tag| sim.consequences.contains(tag))
        && outcome.requires.min_knowledge.iter().all(|floor| {
            sim.subsystems
                .get(&floor.id)
                .is_some_and(|s| s.knowledge >= floor.at_least)
        })
        // Reputation gates (content-depth round 17): a good name or a feared one
        // unlocks a choice a no-name ship cannot reach.
        && outcome
            .requires
            .min_reputation
            .iter()
            .all(|g| sim.reputation(&g.id) >= g.threshold)
        && outcome
            .requires
            .max_reputation
            .iter()
            .all(|g| sim.reputation(&g.id) <= g.threshold)
        // Dominant-faction gate (content-depth factions round 25): a choice only on the
        // table while the named people runs the ship.
        && (outcome.requires.requires_dominant_faction.is_empty()
            || sim.dominant_faction_id()
                == Some(outcome.requires.requires_dominant_faction.as_str()))
}

/// The real indices of the outcomes this ship may currently pick, in authored
/// order (content-depth event families round 12): the modal renders only these,
/// and their positions are the indices `apply_outcome`/`ResolveEvent` expect.
/// Outcome 0 is unconditional by construction (enforced at data-load), so this is
/// never empty.
pub fn available_outcome_indices(sim: &SimState, template: &EventTemplate) -> Vec<usize> {
    template
        .outcomes
        .iter()
        .enumerate()
        .filter(|(_, o)| outcome_available(sim, o))
        .map(|(i, _)| i)
        .collect()
}

/// The band of population impact an outcome may land (real-time loop §3), as a
/// signed `(low, high)` head-count delta — negative for lives lost, positive for
/// arrivals/births. Derived from the outcome's *buffered* `population_delta.count`
/// (the same value `apply_outcome` rolls within, since the sim is paused, so the
/// shown band and the rolled result agree). `None` when the magnitude is below
/// `impact_min_magnitude_for_range` — a small, specific effect shown exactly.
pub fn outcome_pop_impact_range(
    sim: &SimState,
    data: &GameData,
    template: &EventTemplate,
    outcome_index: usize,
) -> Option<(i64, i64)> {
    let outcome = template.outcomes.get(outcome_index)?;
    let (_, _, population) = subsystems::buffered_deltas(
        sim,
        data,
        &template.family,
        outcome.resource_delta,
        outcome.ship_delta,
        outcome.population_delta,
    );
    impact_range(population.count as i64, data.config.real_time)
}

/// The signed `(low, high)` band for a head-count delta, or `None` when it is too
/// small to bother ranging (`impact_min_magnitude_for_range`). Ordered low ≤ high
/// regardless of sign.
fn impact_range(count: i64, cfg: RealTimeConfig) -> Option<(i64, i64)> {
    if count.abs() < cfg.impact_min_magnitude_for_range {
        return None;
    }
    let lo = (count as f32 * (1.0 - cfg.impact_variance)).round() as i64;
    let hi = (count as f32 * (1.0 + cfg.impact_variance)).round() as i64;
    Some((lo.min(hi), lo.max(hi)))
}

/// Roll an actual head-count delta within its impact band (real-time loop §3).
/// A magnitude below the range floor applies exactly; otherwise a uniform draw in
/// `[low, high]` through the seeded RNG.
fn rolled_pop_count(count: i32, cfg: RealTimeConfig, rng: &mut SeededRng) -> i32 {
    match impact_range(count as i64, cfg) {
        Some((lo, hi)) => {
            let span = (hi - lo + 1).max(1) as usize;
            (lo + rng.below(span) as i64) as i32
        }
        None => count,
    }
}

/// An event's description as it should be shown: the template's, plus the riding
/// complication's `description_add` when one is active. Used by the modal so the
/// twist is visible before the player chooses.
pub fn shown_description(sim: &SimState, template: &EventTemplate) -> String {
    match active_complication(sim, template) {
        Some(c) if !c.description_add.is_empty() => {
            format!("{} {}", template.description, c.description_add)
        }
        _ => template.description.clone(),
    }
}

#[cfg(test)]
mod tests;
