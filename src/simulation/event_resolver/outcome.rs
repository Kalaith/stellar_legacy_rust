//! What a choice does: scoring an outcome for the autoplayer, landing its
//! deltas on the sim, and resolving an event with no player at the helm.

use crate::data::events::{EventOutcome, EventTemplate};
use crate::data::{GameConfig, GameData};
use crate::simulation::subsystems;
use crate::state::sim::SimState;

use super::active_complication;
use super::rolled_pop_count;

/// Score an outcome for auto-resolution (GDD §5.4). Higher is better.
pub fn score_outcome(outcome: &EventOutcome, sim: &SimState, config: &GameConfig) -> f32 {
    let food_weight = if sim.resources.food < config.low_food_threshold {
        2.0
    } else {
        1.0
    };
    let ship_distressed = sim.ship.hull_integrity < config.hull_warning_threshold
        || sim.ship.life_support < config.life_support_warning_threshold;
    let ship_weight = if ship_distressed { 1000.0 } else { 100.0 };

    outcome.resource_delta.food as f32 * food_weight
        + (outcome.ship_delta.hull_integrity + outcome.ship_delta.life_support) * ship_weight
        + outcome.resource_delta.credits as f32 * 0.1
        + outcome.resource_delta.energy as f32 * 0.2
        + outcome.resource_delta.minerals as f32 * 0.3
        + outcome.population_delta.morale * 500.0
        + outcome.population_delta.unity * 600.0
        - 100.0 * outcome.long_term_consequences.len() as f32
    // TODO(next agent): + legacy_specific_modifier * 200 once outcomes carry
    // per-legacy modifiers (GDD §5.4).
}

/// Apply one outcome of a pending event to the sim and log it.
pub fn apply_outcome(
    sim: &mut SimState,
    data: &GameData,
    template: &EventTemplate,
    outcome_index: usize,
) {
    let Some(outcome) = template.outcomes.get(outcome_index) else {
        return;
    };
    // Snapshot the riding complication (content-depth round 6) from the state as
    // it stood *before* this outcome — the same state the player saw the twist
    // in — so the outcome's own deltas can't move the gate out from under it.
    let complication = active_complication(sim, template).cloned();
    // A subsystem buffering this event's family softens its harm (W5): every
    // negative delta is scaled down; the boons land in full.
    let (resource_delta, ship_delta, mut population_delta) = subsystems::buffered_deltas(
        sim,
        data,
        &template.family,
        outcome.resource_delta,
        outcome.ship_delta,
        outcome.population_delta,
    );
    // The population toll is uncertain within its shown band (real-time loop §3):
    // roll the actual head-count delta the range promised, through the seeded RNG.
    population_delta.count =
        rolled_pop_count(population_delta.count, data.config.real_time, &mut sim.rng);
    sim.resources.apply(&resource_delta);
    sim.ship.apply(&ship_delta);
    sim.population.apply(&population_delta);
    // A heavy toll may also take a named character (real-time loop follow-up:
    // "a random chance of dying … especially due to an event").
    let population_lost = (-population_delta.count).max(0) as u32;
    crate::simulation::mortality::event_claim(sim, data, population_lost);
    sim.consequences
        .extend(outcome.long_term_consequences.iter().cloned());
    // …and nudge the ship's cumulative character (content-depth round 16): many
    // small reputation moves across a campaign build a lasting tendency.
    for delta in &outcome.reputation_deltas {
        sim.adjust_reputation(&delta.id, delta.delta);
    }
    // …and a promised follow-up joins the clock (content-depth round 9): unlike a
    // consequence tag, this re-fires the named event at a *determined* year, so an
    // authored arc pays off when promised rather than when the RNG obliges.
    if let Some(followup) = &outcome.schedule_followup {
        sim.scheduled_events
            .push(crate::state::sim::ScheduledEvent {
                template_id: followup.template_id.clone(),
                fire_year: sim.year() + followup.delay_years,
            });
    }
    // A salvaged component drops into the hold, to be installed later
    // (PLAN M4.4). The outcome's own log narrates the find.
    if let Some(component_id) = &outcome.grant_component {
        sim.ship.salvage.push(component_id.clone());
    }
    // …and a subsystem version this outcome unlocks (2c): a mission-reward fitting
    // the ship may now build in drydock.
    if let Some(fitting_id) = &outcome.grant_fitting {
        if !sim.ship.unlocked_fittings.contains(fitting_id) {
            sim.ship.unlocked_fittings.push(fitting_id.clone());
        }
    }

    let text = if outcome.log.is_empty() {
        format!("{}: {}", template.title, outcome.label)
    } else {
        outcome.log.clone()
    };
    sim.push_log(text);
    // The council's own answers are the voyage's most-worth-remembering beats,
    // so the homecoming can show the player what they decided a century ago.
    // Only events that actually asked count — an auto-resolved incident was
    // something that happened to the ship, not something it chose.
    if template.requires_decision {
        crate::simulation::debrief::remember(
            sim,
            data,
            crate::state::sim::debrief::HighlightKind::Decision,
            format!("{} — {}", template.title, outcome.label),
        );
    }
    // An outcome may turn the mission for home early (W2) — the outcome's own
    // deltas carry the flavor; this just bends the voyage onto its return leg.
    if outcome.force_return {
        crate::simulation::contract::jump_to_return(sim);
    }
    // …or drive a whole people off the ship (W7) — a named faction for a
    // schism beat (content-depth round 3), else whoever is smallest.
    if let Some(kind) = outcome.faction_loss {
        match &outcome.faction_loss_id {
            Some(id) => sim.apply_faction_loss_by_id(data, kind, id),
            None => sim.apply_faction_loss(data, kind),
        }
    }
    // …or fold two peoples into one (content-depth round 5: assimilation beats).
    // Unlike a schism, the head count is kept — only the name dissolves.
    if let Some(id) = &outcome.faction_merge_id {
        sim.apply_faction_merge(data, id);
    }
    // …or wound / mend / re-teach a subsystem (content-depth coupling): an
    // engineering crisis damages the engineering bay, a teaching succession
    // restores its lost know-how. Unknown ids are ignored.
    for delta in &outcome.subsystem_deltas {
        if let Some(state) = sim.subsystems.get_mut(&delta.id) {
            state.condition = (state.condition + delta.condition).clamp(0.0, 1.0);
            state.knowledge = (state.knowledge + delta.knowledge).clamp(0.0, 1.0);
        }
    }
    // …or earn / spend a people's goodwill (content-depth round 8): the choice
    // shifts named aboard factions' approval, which decides whether a slighted
    // people eventually withdraws. Factions not aboard are ignored.
    for delta in &outcome.faction_approval_deltas {
        if let Some(state) = sim
            .factions
            .iter_mut()
            .find(|f| f.faction_id == delta.id && f.is_aboard())
        {
            state.adjust_approval(delta.delta);
        }
    }
    // …and favoring a people costs you with its rivals (content-depth factions
    // round 14): each approval *gain* spills a fraction of resentment onto the
    // favored people's aboard rivals, so the meter cannot be maxed for everyone.
    sim.apply_rival_approval_spillover(data, &outcome.faction_approval_deltas);
    // …and rewards you with its allies (content-depth factions round 17): the same
    // approval *gain* shares a fraction of goodwill with the favored people's aboard
    // kin, so courting a coalition lifts more than the one people you named.
    sim.apply_ally_approval_spillover(data, &outcome.faction_approval_deltas);
    // …and slighting a people is a small gift to its rivals (content-depth factions
    // round 32): the schadenfreude mirror — each approval *loss* lifts the wounded
    // people's aboard rivals a fraction, completing the rivalry spillover across signs.
    sim.apply_rival_approval_schadenfreude(data, &outcome.faction_approval_deltas);
    // …and a wound to it stings its allies (content-depth factions round 32): the
    // commiseration mirror — each approval *loss* drags the wounded people's aboard kin
    // down a fraction, so a coalition shares its friends' misfortunes as well as favors.
    sim.apply_ally_approval_commiseration(data, &outcome.faction_approval_deltas);
    // …or let the shortage fall on the smallest deck (content-depth provisioning
    // round 8): a rationing triage that spares the many by cutting the fewest
    // sours the people who bore it, resolved dynamically without naming them.
    if outcome.faction_approval_smallest != 0.0 {
        sim.adjust_smallest_faction_approval(outcome.faction_approval_smallest);
    }
    // …or trade the mission for survival, or the reverse (content-depth
    // provisioning round 9): diverting the work crews in a famine slips the
    // charter's tally. A fraction of the objective target, applied only with a
    // contract under way; the objective can slip back but never below zero.
    if outcome.objective_progress_delta != 0.0 {
        if let Some(contract) = sim.contract.as_mut() {
            let shift = outcome.objective_progress_delta * contract.objective_target;
            contract.objective_progress = (contract.objective_progress + shift).max(0.0);
        }
    }
    // …and a riding complication (content-depth round 6) lands its extra toll on
    // top — the event was worse than usual because of the state it arrived in.
    // Round 14: unless the complication targets specific choices, in which case its
    // toll lands only when one of those choices was the one taken.
    let toll_applies = complication.as_ref().is_some_and(|c| {
        c.applies_to_outcomes.is_empty() || c.applies_to_outcomes.contains(&outcome.id)
    });
    if let Some(c) = complication.as_ref().filter(|_| toll_applies) {
        sim.resources.apply(&c.resource_delta);
        sim.ship.apply(&c.ship_delta);
        sim.population.apply(&c.population_delta);
        for delta in &c.subsystem_deltas {
            if let Some(state) = sim.subsystems.get_mut(&delta.id) {
                state.condition = (state.condition + delta.condition).clamp(0.0, 1.0);
                state.knowledge = (state.knowledge + delta.knowledge).clamp(0.0, 1.0);
            }
        }
        if !c.log.is_empty() {
            sim.push_log(c.log.clone());
        }
    }
    // Record this occurrence (content-depth round 11) *after* the complication
    // has read the prior count, so a recurrence complication rides on the Nth
    // time and not the (N+1)th.
    *sim.event_fire_counts
        .entry(template.id.clone())
        .or_default() += 1;
    sim.pending_event = None;
}

/// Pick the best-scoring outcome and apply it (delegated/no-decision path).
/// Returns the applied outcome's label.
pub fn auto_resolve(sim: &mut SimState, data: &GameData, template: &EventTemplate) -> String {
    let best = template
        .outcomes
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            score_outcome(a, sim, &data.config).total_cmp(&score_outcome(b, sim, &data.config))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);
    let label = template
        .outcomes
        .get(best)
        .map(|o| o.label.clone())
        .unwrap_or_default();
    apply_outcome(sim, data, template, best);
    label
}
