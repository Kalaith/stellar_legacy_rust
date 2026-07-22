//! Event rolling, outcome scoring, and resolution (GDD §5.4).

pub mod skeleton;

use crate::data::events::{Complication, EventCategory, EventOutcome, EventTemplate};
use crate::data::{GameConfig, GameData};
use crate::simulation::subsystems;
use crate::state::sim::{PendingEvent, SimState};

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

/// `event_chance = min(cap, base + years_since_event*0.1 + contract_progress*0.2)`.
pub fn event_chance(config: &GameConfig, years_since_event: u32, contract_progress: f32) -> f32 {
    (config.event_chance_base + years_since_event as f32 * 0.1 + contract_progress * 0.2)
        .min(config.event_chance_cap)
}

/// Category weights, scaled up by ship/population distress (GDD §5.4).
pub fn category_weights(sim: &SimState, config: &GameConfig) -> [(EventCategory, f32); 4] {
    let mut crisis = 0.3;
    if sim.resources.food < config.low_food_threshold {
        crisis += 0.2;
    }
    if sim.resources.energy < config.low_energy_threshold {
        crisis += 0.2;
    }
    if sim.ship.hull_integrity < config.hull_warning_threshold {
        crisis += 0.2;
    }
    if sim.ship.life_support < config.life_support_warning_threshold {
        crisis += 0.2;
    }
    if sim.population.morale < 0.5 {
        crisis += 0.15;
    }
    if sim.population.unity < 0.4 {
        crisis += 0.15;
    }
    // Route hazard (content-depth charters round 11): a dangerous writ breeds more
    // crises for its whole voyage — the charter's risk profile, not just the ship's
    // present distress.
    if let Some(contract) = &sim.contract {
        crisis += contract.hazard;
    }

    let milestone = match &sim.contract {
        Some(contract) => {
            let progress = contract.progress();
            if !(0.2..=0.8).contains(&progress) {
                0.4
            } else {
                0.15
            }
        }
        None => 0.05,
    };

    let legacy = (0.1 + (sim.year() / 25) as f32 * 0.05).min(0.3);

    [
        (EventCategory::ImmediateCrisis, crisis),
        (EventCategory::GenerationalChallenge, 0.3),
        (EventCategory::MissionMilestone, milestone),
        (EventCategory::LegacyMoment, legacy),
    ]
}

/// True if `template` clears its W6 phase + voyage gates for the current state:
/// an empty `phases` fires in any phase, otherwise the contract must be active
/// and its current phase listed; year / generation / cultural-drift gates must
/// all be met.
fn passes_gate(sim: &SimState, template: &EventTemplate) -> bool {
    // Scheduled-only payoffs (content-depth round 9) never roll; they fire solely
    // as the timed follow-up of a `schedule_followup`, forced by id past the gates.
    if template.scheduled_only {
        return false;
    }
    if !template.phases.is_empty() {
        match sim.contract.as_ref() {
            Some(contract) if template.phases.contains(&contract.phase) => {}
            _ => return false,
        }
    }
    if !template
        .requires_consequence
        .iter()
        .all(|tag| sim.consequences.contains(tag))
    {
        return false;
    }
    // Consequence bar (content-depth round 13): a disqualifying history closes the
    // door — any forbidden tag on record keeps the event out of the pool.
    if template
        .forbidden_consequence
        .iter()
        .any(|tag| sim.consequences.contains(tag))
    {
        return false;
    }
    if !template.requires_charter_tag.is_empty() {
        match sim.contract.as_ref() {
            Some(contract)
                if template
                    .requires_charter_tag
                    .iter()
                    .all(|tag| contract.tags.contains(tag)) => {}
            _ => return false,
        }
    }
    if !template.requires_dominant_faction.is_empty()
        && sim.dominant_faction_id() != Some(template.requires_dominant_faction.as_str())
    {
        return false;
    }
    if !template
        .requires_factions_aboard
        .iter()
        .all(|id| sim.is_faction_aboard(id))
    {
        return false;
    }
    // Faction-approval gates (content-depth round 8): a grievance/withdrawal beat
    // fires only while the named people is aboard and has soured to its threshold.
    if !template.faction_approval_below.iter().all(|gate| {
        sim.factions
            .iter()
            .any(|f| f.faction_id == gate.id && f.is_aboard() && f.approval <= gate.below)
    }) {
        return false;
    }
    if !template.knowledge_below.iter().all(|gate| {
        sim.subsystems
            .get(&gate.id)
            .is_some_and(|s| s.knowledge <= gate.below)
    }) {
        return false;
    }
    if !template.condition_below.iter().all(|gate| {
        sim.subsystems
            .get(&gate.id)
            .is_some_and(|s| s.condition <= gate.below)
    }) {
        return false;
    }
    if template.food_below.is_some_and(|t| sim.resources.food > t)
        || template.fuel_below.is_some_and(|t| sim.ship.fuel > t)
        || template
            .spare_parts_below
            .is_some_and(|t| sim.ship.spare_parts > t)
        || template
            .energy_below
            .is_some_and(|t| sim.resources.energy > t)
    {
        return false;
    }
    // Abundance gates (content-depth provisioning round 11): the mirror — the
    // event stays out of the pool until the ship is genuinely flush.
    if template.food_above.is_some_and(|t| sim.resources.food < t)
        || template
            .credits_above
            .is_some_and(|t| sim.resources.credits < t)
    {
        return false;
    }
    // Era ceilings (content-depth round 4): 0 = ungated, else the event has
    // passed out of its era once the voyage is beyond the cap.
    if template.max_year != 0 && sim.year() > template.max_year {
        return false;
    }
    if template.max_generation != 0 && sim.dynasty.generation > template.max_generation {
        return false;
    }
    if template.min_objective_fraction > 0.0
        && sim
            .contract
            .as_ref()
            .is_none_or(|c| c.objective_fraction() < template.min_objective_fraction)
    {
        return false;
    }
    // Depopulation gate (content-depth round 12): crew-thinning content stays out
    // of the pool until the crew has fallen to or below its headcount ceiling.
    if template.max_population > 0 && sim.population.count > template.max_population {
        return false;
    }
    // Chronic-scarcity gate (content-depth round 13): long-hunger content waits
    // until the shortage has ground on for years, not just this season.
    if sim.lean_food_years < template.min_lean_food_years {
        return false;
    }
    // Founder-authority gate (content-depth round 14): covenant-lapse content stays
    // out of the pool while the ship still holds the founders' charter binding.
    if template.max_legacy_loyalty > 0.0
        && sim.population.legacy_loyalty > template.max_legacy_loyalty
    {
        return false;
    }
    sim.year() >= template.min_year
        && sim.dynasty.generation >= template.min_generation
        && sim.population.cultural_drift >= template.min_cultural_drift
        && sim.population.morale >= template.min_morale
        && sim.population.unity >= template.min_unity
}

/// Weighted pick among already gate-cleared candidates (sorted by id for
/// determinism): legacy affinity × the buffering subsystem's rarefying factor
/// (W5). Records the fire on the sim and returns the pending event, or `None`
/// when nothing survived the filter.
fn pick_weighted(
    sim: &mut SimState,
    data: &GameData,
    mut candidates: Vec<(&String, &EventTemplate)>,
) -> Option<PendingEvent> {
    candidates.sort_by(|a, b| a.0.cmp(b.0));
    if candidates.is_empty() {
        return None;
    }
    let legacy_id = sim.legacy.legacy_id.as_str();
    let template_weights: Vec<f32> = candidates
        .iter()
        .map(|(_, t)| {
            *t.legacy_weight_modifiers.get(legacy_id).unwrap_or(&1.0)
                * subsystems::family_weight_factor(sim, data, &t.family)
        })
        .collect();
    let weight_total: f32 = template_weights.iter().sum();
    let mut roll = sim.rng.next_f32() * weight_total;
    let mut chosen = candidates[0].1;
    for (i, weight) in template_weights.iter().enumerate() {
        if roll < *weight {
            chosen = candidates[i].1;
            break;
        }
        roll -= weight;
    }
    sim.last_event_month_clock = sim.month_clock;
    Some(PendingEvent {
        template_id: chosen.id.clone(),
        rolled_month_clock: sim.month_clock,
    })
}

/// Roll for a reactive/filler event (W6): the monthly chance, a category by
/// weight, then a gate-cleared template within it. Returns the pending event
/// without applying anything; the caller decides block vs auto-resolve.
pub fn roll_event(sim: &mut SimState, data: &GameData) -> Option<PendingEvent> {
    let progress = sim.contract.as_ref().map_or(0.0, |c| c.progress());
    // The ramp is still a per-year model; convert its whole-year gap and the
    // resulting yearly chance to a per-month roll so expected events per year
    // is preserved while events can now fire (and be dated) any month (W3).
    let years_since = sim.month_clock.saturating_sub(sim.last_event_month_clock) / 12;
    let monthly_chance = event_chance(&data.config, years_since, progress) / 12.0;
    if !sim.rng.chance(monthly_chance) {
        return None;
    }

    // Pick a category by weight; candidates are that category's gate-cleared
    // templates (W6 phase/year/generation/drift filters).
    let weights = category_weights(sim, &data.config);
    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    let mut pick = sim.rng.next_f32() * total;
    let mut category = EventCategory::ImmediateCrisis;
    for (cat, weight) in weights {
        if pick < weight {
            category = cat;
            break;
        }
        pick -= weight;
    }

    let candidates: Vec<(&String, &EventTemplate)> = data
        .events
        .iter()
        .filter(|(_, t)| t.category == category && passes_gate(sim, t))
        .collect();
    pick_weighted(sim, data, candidates)
}

/// Roll a scheduled beat's event (W6): no chance roll — a beat always fires —
/// filtering the catalog to `family` plus the W6 gates, then the normal
/// weighting. `None` when the family is over-gated (caller falls through).
pub fn roll_event_in_family(
    sim: &mut SimState,
    data: &GameData,
    family: &str,
) -> Option<PendingEvent> {
    let candidates: Vec<(&String, &EventTemplate)> = data
        .events
        .iter()
        .filter(|(_, t)| t.family == family && passes_gate(sim, t))
        .collect();
    pick_weighted(sim, data, candidates)
}

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
    let (resource_delta, ship_delta, population_delta) = subsystems::buffered_deltas(
        sim,
        data,
        &template.family,
        outcome.resource_delta,
        outcome.ship_delta,
        outcome.population_delta,
    );
    sim.resources.apply(&resource_delta);
    sim.ship.apply(&ship_delta);
    sim.population.apply(&population_delta);
    sim.consequences
        .extend(outcome.long_term_consequences.iter().cloned());
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

    let text = if outcome.log.is_empty() {
        format!("{}: {}", template.title, outcome.label)
    } else {
        outcome.log.clone()
    };
    sim.push_log(text);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn a_cultural_drift_gate_holds_a_template_until_the_drift_arrives() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 1, &picks);
        // The Long Schism is gated at min_cultural_drift 0.6 (W6).
        let schism = data.events.get("the_schism_deepens").unwrap();
        assert!((schism.min_cultural_drift - 0.6).abs() < 1e-6);

        sim.population.cultural_drift = 0.2;
        assert!(
            !passes_gate(&sim, schism),
            "the schism stays out of the pool below its drift gate"
        );
        sim.population.cultural_drift = 0.7;
        assert!(
            passes_gate(&sim, schism),
            "the schism enters the pool once drift is high enough"
        );
    }

    #[test]
    fn trilemma_events_offer_a_genuinely_distinct_third_path() {
        // Content-depth event-families round 8: the set was overwhelmingly binary
        // (175/189 events had exactly two outcomes). Five iconic dilemmas gained a
        // real third path — each a different strategic axis, not a milquetoast
        // middle. This locks that they resolve as three legal, distinct outcomes.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        for id in [
            "tithe_demand",
            "micrometeoroid_storm",
            "cultural_schism",
            "skills_drought",
            "the_wary_frontier",
        ] {
            let event = data.events.get(id).unwrap();
            assert_eq!(event.outcomes.len(), 3, "{id} should be a trilemma now");
        }

        // The tithe's third path (offer service) is materially distinct from the
        // other two: unlike paying it spends no hard credits, unlike running it
        // takes no hull damage, and it earns influence the ship would not get by
        // either. Apply it from a clean state and check those effects land.
        let event = data.events.get("tithe_demand").unwrap();
        let idx = event
            .outcomes
            .iter()
            .position(|o| o.id == "offer_service")
            .unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 12, &picks);
        let credits_before = sim.resources.credits;
        let influence_before = sim.resources.influence;
        let hull_before = sim.ship.hull_integrity;
        apply_outcome(&mut sim, &data, event, idx);
        assert_eq!(
            sim.resources.credits, credits_before,
            "offering service costs no treasury"
        );
        assert!(
            sim.resources.influence > influence_before,
            "competence-for-passage earns standing"
        );
        assert_eq!(
            sim.ship.hull_integrity, hull_before,
            "no shots fired, no hull lost"
        );
    }

    #[test]
    fn a_consequence_gate_holds_the_payoff_until_the_setup_choice_fires() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 5, &picks);
        // `the_ward_reopens` is the payoff half of the `sealed_ward` chain
        // (content-depth iteration): it may not fire until sealing the ward
        // recorded that consequence.
        let payoff = data.events.get("the_ward_reopens").unwrap();
        assert_eq!(payoff.requires_consequence, vec!["sealed_ward".to_string()]);
        sim.dynasty.generation = 5; // clear its min_generation gate

        assert!(
            !passes_gate(&sim, payoff),
            "the reopening stays out of the pool before the ward was ever sealed"
        );
        sim.consequences.push("sealed_ward".to_string());
        assert!(
            passes_gate(&sim, payoff),
            "sealing the ward unlocks the reopening decades later"
        );
    }

    #[test]
    fn the_ghost_signal_schedules_its_own_appointed_hour() {
        // Content-depth event families round 10: the predestination loop, closed
        // with the round-9 scheduling. Answering the ghost signal — the ship's own
        // call sign timestamped for a future year — schedules that year's reckoning,
        // and the payoff is scheduled_only so it fires only when its date arrives.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "wanderers", 41, &picks);
        sim.dynasty.generation = 2; // an age past the ghost's own drift complication is irrelevant here

        let ghost = data.events.get("ghost_signal").unwrap();
        let payoff = data.events.get("the_appointed_signal").unwrap();
        assert!(
            payoff.scheduled_only,
            "the appointed signal must never roll on its own"
        );
        let answer = ghost
            .outcomes
            .iter()
            .position(|o| o.id == "answer_the_ghost")
            .unwrap();
        let delay = ghost.outcomes[answer]
            .schedule_followup
            .as_ref()
            .expect("answering the ghost schedules its return")
            .delay_years;

        let year0 = sim.year();
        apply_outcome(&mut sim, &data, ghost, answer);
        assert_eq!(sim.scheduled_events.len(), 1, "answering queues the payoff");
        assert_eq!(sim.scheduled_events[0].template_id, "the_appointed_signal");
        assert_eq!(
            sim.scheduled_events[0].fire_year,
            year0 + delay,
            "the loop is set for the year the signal named"
        );
    }

    #[test]
    fn deferred_maintenance_comes_due_a_generation_on() {
        // Content-depth event families round 10: completing a dangling thread. The
        // "defer the fix" outcomes of three engineering crises recorded a debt no
        // event ever collected; now it comes due a generation later.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 8, &picks);
        let bill = data.events.get("the_bill_comes_due").unwrap();
        assert_eq!(
            bill.requires_consequence,
            vec!["deferred_maintenance".to_string()]
        );
        sim.dynasty.generation = 5; // clear its min_generation

        assert!(
            !passes_gate(&sim, bill),
            "no reckoning for a ship that never deferred"
        );
        sim.consequences.push("deferred_maintenance".to_string());
        assert!(
            passes_gate(&sim, bill),
            "the deferred ledger comes due once it is on record"
        );
    }

    #[test]
    fn a_charted_dearth_arrives_on_its_date_softened_only_if_provisioned() {
        // Content-depth provisioning round 10: foresight on a determined clock.
        // Charting the dearth schedules its guaranteed arrival; laying in stores
        // seeds the consequence the payoff's complication reads to soften it; the
        // payoff itself is scheduled-only and never rolls on its own.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 33, &picks);

        let setup = data.events.get("the_charted_dearth").unwrap();
        let payoff = data.events.get("the_dearth_arrives").unwrap();
        assert!(
            payoff.scheduled_only,
            "the dearth fires only when its charted year comes"
        );
        // Its relief complication rides on the laid-in-stores consequence.
        let comp = payoff
            .complications
            .iter()
            .find(|c| {
                c.requires_consequence
                    .contains(&"laid_in_for_dearth".to_string())
            })
            .expect("a relief complication for the provisioned ship");

        // Laying in stores queues the dearth *and* records the foresight.
        let lay_in = setup
            .outcomes
            .iter()
            .position(|o| o.id == "lay_in_stores")
            .unwrap();
        let delay = setup.outcomes[lay_in]
            .schedule_followup
            .as_ref()
            .unwrap()
            .delay_years;
        let year0 = sim.year();
        apply_outcome(&mut sim, &data, setup, lay_in);
        assert_eq!(sim.scheduled_events[0].template_id, "the_dearth_arrives");
        assert_eq!(sim.scheduled_events[0].fire_year, year0 + delay);
        assert!(
            sim.consequences.contains(&"laid_in_for_dearth".to_string()),
            "laying in is on record for the complication to find"
        );

        // With the foresight on record, the relief complication rides the payoff.
        assert!(
            active_complication(&sim, payoff).is_some_and(|c| c.id == comp.id),
            "the laid-in stores answer the dearth"
        );

        // A ship that trusted to slack has no such relief.
        let mut unready = SimState::new_campaign(&data, "preservers", 33, &picks);
        assert!(
            active_complication(&unready, payoff).is_none(),
            "an unprovisioned ship meets the dearth bare"
        );
        // And trusting the slack still schedules the (unsoftened) dearth.
        let trust = setup
            .outcomes
            .iter()
            .position(|o| o.id == "trust_the_slack")
            .unwrap();
        apply_outcome(&mut unready, &data, setup, trust);
        assert_eq!(sim.scheduled_events.len(), 1);
        assert!(
            !unready
                .consequences
                .contains(&"laid_in_for_dearth".to_string()),
            "trusting the slack lays in nothing"
        );
    }

    #[test]
    fn a_famine_can_be_answered_by_slipping_the_mission_or_holding_to_it() {
        // Content-depth provisioning round 9: the founders' mission and the
        // living's survival compete. Diverting the work crews feeds the ship but
        // slips the charter's objective; holding to the work keeps the tally whole
        // and lets the shortage bite. The objective only moves with a contract.
        use crate::data::contracts::ContractPhase;
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 29, &picks);
        let event = data.events.get("the_fallow_season").unwrap();

        // On-station, and genuinely short: the choice is forced.
        let template = data.contracts.get("deep_vein_survey").unwrap();
        let mut active = crate::simulation::contract::start_contract(template, &sim);
        active.phase = ContractPhase::Operation;
        active.objective_progress = active.objective_target * 0.5;
        sim.contract = Some(active);
        let famine = event.food_below.unwrap();
        sim.resources.food = famine + 1;
        assert!(
            !passes_gate(&sim, event),
            "a stocked larder holds no dilemma"
        );
        sim.resources.food = famine - 1;
        assert!(
            passes_gate(&sim, event),
            "a real shortfall on station forces it"
        );

        let obj_before = sim.contract.as_ref().unwrap().objective_progress;
        let food_before = sim.resources.food;

        // Diverting the crews feeds the ship and slips the tally.
        let mut divert = sim.clone();
        let d = event
            .outcomes
            .iter()
            .position(|o| o.id == "divert_the_crews")
            .unwrap();
        apply_outcome(&mut divert, &data, event, d);
        assert!(
            divert.resources.food > food_before,
            "diverting the crews feeds the ship"
        );
        assert!(
            divert.contract.as_ref().unwrap().objective_progress < obj_before,
            "the mission's tally slips when the crews leave the work"
        );

        // Holding to the work keeps the objective exactly where it was.
        let mut hold = sim.clone();
        let h = event
            .outcomes
            .iter()
            .position(|o| o.id == "hold_to_the_work")
            .unwrap();
        apply_outcome(&mut hold, &data, event, h);
        assert_eq!(
            hold.contract.as_ref().unwrap().objective_progress,
            obj_before,
            "holding to the founders' work leaves the tally untouched"
        );
    }

    #[test]
    fn a_shortage_triage_sours_the_deck_that_bears_the_cut() {
        // Content-depth provisioning round 8: the "who bears the cut" coupling.
        // Rationing the shortfall onto the smallest deck sours that people
        // (feeding the round-8 withdrawal); sharing the cut equally sours no one.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 23, &picks);

        // Identify the smallest aboard people and its launch approval.
        let smallest_id = sim
            .factions
            .iter()
            .filter(|f| f.is_aboard())
            .min_by(|a, b| {
                a.members
                    .cmp(&b.members)
                    .then_with(|| a.faction_id.cmp(&b.faction_id))
            })
            .unwrap()
            .faction_id
            .clone();
        let approval_of = |sim: &SimState, id: &str| {
            sim.factions
                .iter()
                .find(|f| f.faction_id == id)
                .unwrap()
                .approval
        };
        let before = approval_of(&sim, &smallest_id);

        let event = data.events.get("the_thin_table").unwrap();
        // It gates on a genuine shortage.
        let famine = event.food_below.unwrap();
        sim.resources.food = famine + 1;
        assert!(!passes_gate(&sim, event), "a stocked larder is not triaged");
        sim.resources.food = famine - 1;
        assert!(
            passes_gate(&sim, event),
            "a real shortfall forces the choice"
        );

        // Sharing the cut equally leaves every people's standing intact.
        let mut fair = sim.clone();
        let share = event
            .outcomes
            .iter()
            .position(|o| o.id == "share_evenly")
            .unwrap();
        apply_outcome(&mut fair, &data, event, share);
        assert_eq!(
            approval_of(&fair, &smallest_id),
            before,
            "an equal cut sours no one in particular"
        );

        // Cutting the smallest deck first sours precisely that people.
        let cut = event
            .outcomes
            .iter()
            .position(|o| o.id == "cut_the_smallest")
            .unwrap();
        apply_outcome(&mut sim, &data, event, cut);
        assert!(
            approval_of(&sim, &smallest_id) < before,
            "the deck that bore the cut remembers it"
        );
    }

    #[test]
    fn faction_approval_gates_a_slighted_peoples_withdrawal() {
        // Content-depth factions round 8: the reserved approval mechanic. Event
        // choices earn or spend a people's goodwill, and a faction slighted past
        // a threshold generates its own withdrawal — so *how you treat a people*,
        // not only how far the voyage has drifted, decides whether it stays.
        use crate::state::sim::factions::{FactionState, FactionStatus};
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 19, &picks);
        sim.dynasty.generation = 4; // clear the withdrawal's min_generation

        // Ensure the First Flame is aboard at the launch midpoint.
        if sim.factions.iter().all(|f| f.faction_id != "first_flame") {
            sim.factions.push(FactionState {
                faction_id: "first_flame".to_string(),
                members: 300,
                status: FactionStatus::Aboard,
                approval: crate::state::sim::factions::default_approval(),
                mood_band: 0,
            });
            sim.population.count += 300;
        }
        let flame_approval = |sim: &SimState| {
            sim.factions
                .iter()
                .find(|f| f.faction_id == "first_flame")
                .map(|f| f.approval)
        };
        assert_eq!(flame_approval(&sim), Some(0.5), "a people launches neutral");

        let petition = data.events.get("the_flame_petition").unwrap();
        let withdrawal = data.events.get("the_flame_withdrawal").unwrap();

        // The grievance fires whenever the Flame is aboard; the withdrawal waits
        // until they have actually soured.
        assert!(
            passes_gate(&sim, petition),
            "the Keepers can always petition"
        );
        assert!(
            !passes_gate(&sim, withdrawal),
            "a content people does not withdraw"
        );

        // Slight them once — approval drops but not yet past the threshold.
        let slight = petition
            .outcomes
            .iter()
            .position(|o| o.id == "hold_the_line")
            .unwrap();
        apply_outcome(&mut sim, &data, petition, slight);
        assert!(
            flame_approval(&sim).unwrap() < 0.5,
            "the slight is remembered"
        );
        assert!(
            !passes_gate(&sim, withdrawal),
            "one slight is not yet a departure"
        );

        // Slight them again — now they have soured past the threshold and the
        // withdrawal enters the pool.
        apply_outcome(&mut sim, &data, petition, slight);
        assert!(
            passes_gate(&sim, withdrawal),
            "a people slighted past the threshold moves to leave"
        );

        // Paying to keep them lifts approval back above the line and closes the
        // withdrawal (the loop can recover).
        let mut kept = sim.clone();
        let beg = withdrawal
            .outcomes
            .iter()
            .position(|o| o.id == "beg_them_stay")
            .unwrap();
        apply_outcome(&mut kept, &data, withdrawal, beg);
        assert!(kept.is_faction_aboard("first_flame"), "bought back aboard");
        assert!(
            !passes_gate(&kept, withdrawal),
            "goodwill restored closes the withdrawal"
        );

        // Or letting them go actually sheds the people.
        let go = withdrawal
            .outcomes
            .iter()
            .position(|o| o.id == "let_them_go")
            .unwrap();
        apply_outcome(&mut sim, &data, withdrawal, go);
        assert!(
            !sim.is_faction_aboard("first_flame"),
            "the slighted people departs"
        );
    }

    #[test]
    fn the_embassy_pool_colors_only_inhabited_charters() {
        // Content-depth charters round 8: the embassy/inhabited mission kind
        // finally has a signature event pool (mirroring round 6's stellar_hazard
        // pool), and the objective vocabulary gained Diplomacy/Salvage so the
        // charter card names an embassy an embassy, not a rescue.
        use crate::data::contracts::{ContractObjective, ContractPhase};
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 31, &picks);

        // The reclassified charters carry their true objective now.
        assert_eq!(
            data.contracts.get("hearthfall_accord").unwrap().objective,
            ContractObjective::Diplomacy,
            "an eight-generation embassy is Diplomacy, not Rescue"
        );
        assert_eq!(
            data.contracts.get("the_long_tow").unwrap().objective,
            ContractObjective::Salvage,
            "hauling a dead titan-ship is Salvage, not Mining"
        );

        let residency = data.events.get("the_long_residency").unwrap();
        assert_eq!(
            residency.requires_charter_tag,
            vec!["inhabited".to_string()]
        );

        // On an embassy, deep into the residency: the pool fires.
        let embassy = data.contracts.get("hearthfall_accord").unwrap();
        assert!(embassy.tags.contains(&"inhabited".to_string()));
        let mut active = crate::simulation::contract::start_contract(embassy, &sim);
        active.phase = ContractPhase::Operation;
        sim.contract = Some(active);
        sim.dynasty.generation = 6; // clear the residency's min_generation
        assert!(
            passes_gate(&sim, residency),
            "the long residency fires on an inhabited charter, on station"
        );

        // In transit to the embassy, it holds out — the residency is on-station.
        sim.contract.as_mut().unwrap().phase = ContractPhase::Travel;
        assert!(
            !passes_gate(&sim, residency),
            "there is no residency until the ship is living among them"
        );

        // A mining charter never hosts an embassy beat.
        let mining = data.contracts.get("deep_vein_survey").unwrap();
        assert!(!mining.tags.contains(&"inhabited".to_string()));
        let mut active = crate::simulation::contract::start_contract(mining, &sim);
        active.phase = ContractPhase::Operation;
        sim.contract = Some(active);
        assert!(
            !passes_gate(&sim, residency),
            "a cinder-vein camp has no host people"
        );
    }

    #[test]
    fn the_stellar_hazard_pool_colors_only_its_destination() {
        // Content-depth charters round 6: the stellar_hazard destination finally
        // has a signature event pool. Its beats fire on a stellar_hazard
        // charter's Operation and nowhere else — the charter-specific-pool
        // contract that colors coronal_tap and the new sunward dive.
        use crate::data::contracts::ContractPhase;
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 47, &picks);
        let flare = data.events.get("the_coronal_flare").unwrap();
        assert_eq!(
            flare.requires_charter_tag,
            vec!["stellar_hazard".to_string()]
        );

        // A star-diving charter, on station: the flare can strike.
        let dive = data.contracts.get("the_sunward_dive").unwrap();
        assert!(dive.tags.contains(&"stellar_hazard".to_string()));
        let mut active = crate::simulation::contract::start_contract(dive, &sim);
        active.phase = ContractPhase::Operation;
        sim.contract = Some(active);
        assert!(
            passes_gate(&sim, flare),
            "on station near the star, it fires"
        );

        // The same charter in transit (Travel) — the danger is being *at* the
        // star, so the operation-phase gate holds it out.
        sim.contract.as_mut().unwrap().phase = ContractPhase::Travel;
        assert!(
            !passes_gate(&sim, flare),
            "the flare only reaches on-station"
        );

        // A deep-space survey with no stellar hazard never sees it.
        let veiled = data.contracts.get("veiled_expanse_survey").unwrap();
        assert!(!veiled.tags.contains(&"stellar_hazard".to_string()));
        let mut active = crate::simulation::contract::start_contract(veiled, &sim);
        active.phase = ContractPhase::Operation;
        sim.contract = Some(active);
        assert!(!passes_gate(&sim, flare), "a starless survey never flares");
    }

    #[test]
    fn a_charter_tag_gate_keys_an_event_to_its_destination() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 7, &picks);
        // `boarding_alarm` is keyed to hostile-space charters.
        let event = data.events.get("boarding_alarm").unwrap();
        assert_eq!(
            event.requires_charter_tag,
            vec!["hostile_space".to_string()]
        );

        // No contract: a charter-tagged event cannot fire.
        assert!(!passes_gate(&sim, event));

        // A hostile-space charter carries the tag onto the active contract.
        let template = data.contracts.get("warden_patrol").unwrap();
        assert!(template.tags.contains(&"hostile_space".to_string()));
        let mut active = crate::simulation::contract::start_contract(template, &sim);
        active.phase = crate::data::contracts::ContractPhase::Travel;
        sim.contract = Some(active);
        assert!(
            passes_gate(&sim, event),
            "a hostile-space charter unlocks the boarding scare"
        );

        // A colony charter without the tag does not.
        let peaceful = data.contracts.get("seedfall").unwrap();
        assert!(!peaceful.tags.contains(&"hostile_space".to_string()));
        let mut active = crate::simulation::contract::start_contract(peaceful, &sim);
        active.phase = crate::data::contracts::ContractPhase::Travel;
        sim.contract = Some(active);
        assert!(!passes_gate(&sim, event));
    }

    #[test]
    fn a_dominant_faction_gate_colors_events_by_who_runs_the_ship() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 9, &picks);
        // `the_rewriting` is Ascension-Circle-flavored augmentation zealotry.
        let event = data.events.get("the_rewriting").unwrap();
        assert_eq!(event.requires_dominant_faction, "ascension_circle");
        sim.dynasty.generation = 3; // clear its min_generation gate

        // Make the Ascension Circle the clear majority aboard.
        for f in &mut sim.factions {
            f.members = if f.faction_id == "ascension_circle" {
                900
            } else {
                50
            };
        }
        assert_eq!(sim.dominant_faction_id(), Some("ascension_circle"));
        assert!(passes_gate(&sim, event));

        // Shift dominance elsewhere: the event drops out of the pool.
        for f in &mut sim.factions {
            f.members = if f.faction_id == "ascension_circle" {
                50
            } else {
                900
            };
        }
        assert_ne!(sim.dominant_faction_id(), Some("ascension_circle"));
        assert!(!passes_gate(&sim, event));
    }

    #[test]
    fn a_knowledge_crisis_gates_on_low_know_how_and_its_outcome_reteaches_it() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 11, &picks);
        let event = data.events.get("the_last_engineer").unwrap();
        assert_eq!(event.knowledge_below[0].id, "engineering_bay");

        // Healthy know-how: the crisis stays out of the pool.
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.8;
        assert!(!passes_gate(&sim, event));

        // Once knowledge has decayed under the threshold, it can fire.
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.2;
        assert!(passes_gate(&sim, event));

        // Applying the apprentice outcome re-teaches the bay (knowledge +0.35).
        let before = sim.subsystems["engineering_bay"].knowledge;
        apply_outcome(&mut sim, &data, event, 0);
        let after = sim.subsystems["engineering_bay"].knowledge;
        assert!(
            after > before,
            "the teaching succession restores lost know-how ({before} -> {after})"
        );
    }

    #[test]
    fn the_wandering_mind_gates_on_lost_know_how_and_its_choices_diverge() {
        // Content-depth event-families round 4: a mystery gated on the same
        // engineering knowledge decay, whose two outcomes push that knowledge in
        // opposite directions — trusting the old system erodes understanding,
        // rebuilding it by hand restores it. The choice must genuinely matter.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 3, &picks);
        let event = data.events.get("the_wandering_mind").unwrap();
        assert_eq!(event.knowledge_below[0].id, "engineering_bay");
        sim.dynasty.generation = 3; // clear its min_generation gate

        // Healthy know-how: the mystery stays out of the pool.
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.8;
        assert!(!passes_gate(&sim, event));
        // Decayed: it can fire.
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.2;
        assert!(passes_gate(&sim, event));

        // Outcome 0 (trust it) erodes knowledge; outcome 1 (rebuild) restores it.
        let mut trusting = sim.clone();
        apply_outcome(&mut trusting, &data, event, 0);
        let mut rebuilding = sim.clone();
        apply_outcome(&mut rebuilding, &data, event, 1);
        assert!(
            trusting.subsystems["engineering_bay"].knowledge
                < rebuilding.subsystems["engineering_bay"].knowledge,
            "obeying the old mind should cost understanding that rebuilding restores"
        );
    }

    #[test]
    fn an_assimilation_beat_folds_a_people_in_without_losing_them() {
        // Content-depth factions round 5: the union counterpart to a schism. The
        // merger dissolves the named faction's separate identity but keeps its
        // people aboard — head count untouched, its members folded into the host
        // — where a fracture would have dropped them off the ship entirely.
        let data = GameData::load().unwrap();
        let picks = vec![
            "hearth_union".to_string(),
            "verdant_kin".to_string(),
            "meridian_accord".to_string(),
        ];
        let mut sim = SimState::new_campaign(&data, "preservers", 55, &picks);
        sim.dynasty.generation = 6;
        sim.population.cultural_drift = 0.5;

        let event = data.events.get("the_green_hearth").unwrap();
        assert!(passes_gate(&sim, event), "the union fires with both aboard");
        let bless = event
            .outcomes
            .iter()
            .position(|o| o.faction_merge_id.as_deref() == Some("verdant_kin"))
            .expect("the green hearth can bless the union");

        let heads_before = sim.population.count;
        let kin_members = sim
            .factions
            .iter()
            .find(|f| f.faction_id == "verdant_kin")
            .map(|f| f.members)
            .unwrap();
        assert!(kin_members > 0);
        apply_outcome(&mut sim, &data, event, bless);

        assert!(
            !sim.is_faction_aboard("verdant_kin"),
            "the merged people lose their separate name"
        );
        assert!(
            sim.is_faction_aboard("hearth_union"),
            "the host people remain"
        );
        assert_eq!(
            sim.population.count, heads_before,
            "a union keeps every soul aboard — unlike a schism, which sheds them"
        );
    }

    #[test]
    fn a_friction_fracture_sheds_the_named_faction_and_its_craft() {
        // Content-depth factions round 4: an inter-faction quarrel that gates on
        // BOTH factions being aboard and whose "let it break" outcome sheds the
        // named one via faction_loss_id AND carries its subsystem coupling — the
        // machinists take their engineering know-how with them when they go.
        let data = GameData::load().unwrap();
        // Found a campaign that actually holds the quarrelling pair aboard.
        let picks = vec![
            "steel_covenant".to_string(),
            "verdant_kin".to_string(),
            "hearth_union".to_string(),
        ];
        let mut sim = SimState::new_campaign(&data, "adaptors", 41, &picks);
        sim.dynasty.generation = 5;
        sim.population.cultural_drift = 0.6;

        let event = data.events.get("the_forge_and_the_garden").unwrap();
        assert!(
            passes_gate(&sim, event),
            "the quarrel fires with both aboard"
        );
        // Make the Covenant the LARGEST, so a shed-the-smallest rule would spare
        // it — proving the fracture targets the named faction, not the smallest.
        for f in &mut sim.factions {
            f.members = if f.faction_id == "steel_covenant" {
                900
            } else {
                100
            };
        }
        let before = sim.subsystems["engineering_bay"].knowledge;

        let fracture = event
            .outcomes
            .iter()
            .position(|o| o.faction_loss_id.as_deref() == Some("steel_covenant"))
            .expect("the forge quarrel can end in the Covenant leaving");
        apply_outcome(&mut sim, &data, event, fracture);

        assert!(
            !sim.is_faction_aboard("steel_covenant"),
            "the named faction departs even as the largest aboard"
        );
        assert!(
            sim.is_faction_aboard("verdant_kin"),
            "the other quarreller stays"
        );
        assert!(
            sim.subsystems["engineering_bay"].knowledge < before,
            "the machinists' craft leaves with them"
        );
    }

    #[test]
    fn a_faction_colored_complication_rides_only_under_its_faction() {
        // Content-depth factions round 6: the same crisis reads differently
        // depending on who runs the ship. micrometeoroid_storm gains a First
        // Flame reaction (a trial of faith) only while the Keepers are dominant.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 88, &picks);
        let template = data.events.get("micrometeoroid_storm").unwrap();
        let comp = template
            .complications
            .iter()
            .find(|c| c.requires_dominant_faction == "first_flame")
            .expect("the storm carries a First Flame reaction");
        assert!(
            sim.is_faction_aboard("first_flame"),
            "seed campaign holds the Flame"
        );

        // Someone else dominant: the faction reaction stays out.
        for f in &mut sim.factions {
            f.members = if f.faction_id == "first_flame" {
                50
            } else {
                900
            };
        }
        assert_ne!(sim.dominant_faction_id(), Some("first_flame"));
        assert!(active_complication(&sim, template).is_none());

        // The Keepers running the ship: the reaction rides and shows.
        for f in &mut sim.factions {
            f.members = if f.faction_id == "first_flame" {
                900
            } else {
                50
            };
        }
        assert_eq!(sim.dominant_faction_id(), Some("first_flame"));
        assert_eq!(
            active_complication(&sim, template).map(|c| &c.id),
            Some(&comp.id)
        );
        assert!(shown_description(&sim, template).contains("Keepers"));
    }

    #[test]
    fn an_event_with_two_complications_rides_the_first_that_matches() {
        // Content-depth event families round 7: the doc's "2-3 complications is
        // worth three flat events." system_failure now carries two — a failing
        // engineering bay (first) and a Steel Covenant reaction (second). The
        // first whose gates hold rides, so a worn bay wins even when the Covenant
        // is in charge, and the Covenant's is what shows on a sound ship.
        let data = GameData::load().unwrap();
        let picks = vec![
            "steel_covenant".to_string(),
            "hearth_union".to_string(),
            "meridian_accord".to_string(),
        ];
        let template = data.events.get("system_failure").unwrap();
        assert_eq!(template.complications.len(), 2);

        // Steel Covenant running a sound ship: their reaction rides.
        let mut covenant = SimState::new_campaign(&data, "adaptors", 71, &picks);
        for f in &mut covenant.factions {
            f.members = if f.faction_id == "steel_covenant" {
                900
            } else {
                50
            };
        }
        covenant
            .subsystems
            .get_mut("engineering_bay")
            .unwrap()
            .condition = 0.9;
        assert_eq!(
            active_complication(&covenant, template).map(|c| c.id.as_str()),
            Some("covenant_takes_it_in_hand")
        );

        // Same ship, but the bay is failing: the earlier complication wins.
        let mut failing = covenant.clone();
        failing
            .subsystems
            .get_mut("engineering_bay")
            .unwrap()
            .condition = 0.2;
        assert_eq!(
            active_complication(&failing, template).map(|c| c.id.as_str()),
            Some("bay_already_failing"),
            "the first matching complication takes precedence"
        );
    }

    #[test]
    fn a_hazardous_charter_breeds_more_crises_than_a_quiet_one() {
        // Content-depth charters round 11: a charter's route hazard is its risk
        // profile, added to the immediate-crisis category weight for the voyage —
        // a lawless derelict field breeds more crises than a quiet survey, by
        // exactly the charter's hazard.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 51, &picks);

        let calm = data.contracts.get("deep_vein_survey").unwrap().clone();
        let dangerous = data.contracts.get("hollow_fleet").unwrap().clone();
        assert_eq!(calm.hazard, 0.0, "a survey is an ordinary route");
        assert!(dangerous.hazard > 0.0, "a derelict field is a risk profile");

        let crisis_weight = |sim: &SimState| {
            category_weights(sim, &data.config)
                .iter()
                .find(|(c, _)| *c == EventCategory::ImmediateCrisis)
                .unwrap()
                .1
        };

        sim.contract = Some(crate::simulation::contract::start_contract(&calm, &sim));
        let calm_w = crisis_weight(&sim);
        sim.contract = Some(crate::simulation::contract::start_contract(
            &dangerous, &sim,
        ));
        let dangerous_w = crisis_weight(&sim);

        assert!(
            dangerous_w > calm_w,
            "a hazardous route breeds more crises: {dangerous_w} vs {calm_w}"
        );
        assert!(
            (dangerous_w - calm_w - dangerous.hazard).abs() < 1e-5,
            "the crisis bump is exactly the charter's hazard"
        );
    }

    #[test]
    fn a_recurring_crisis_escalates_only_after_prior_occurrences() {
        // Content-depth event families round 11: a recurring crisis escalates
        // instead of merely repeating. Contagion's weariness complication rides
        // only once the same plague has already walked the decks twice before —
        // and resolving the event records each occurrence.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 37, &picks);
        let contagion = data.events.get("contagion").unwrap();
        let comp = contagion
            .complications
            .iter()
            .find(|c| c.min_prior_occurrences >= 2)
            .expect("contagion carries a recurrence complication");

        // First and second time: no escalation yet.
        assert!(
            active_complication(&sim, contagion).is_none(),
            "the first outbreak is just an outbreak"
        );
        apply_outcome(&mut sim, &data, contagion, 0);
        assert_eq!(sim.event_fire_counts["contagion"], 1);
        assert!(
            active_complication(&sim, contagion).is_none(),
            "the second is still not the weariness"
        );
        apply_outcome(&mut sim, &data, contagion, 0);
        assert_eq!(sim.event_fire_counts["contagion"], 2);

        // Third time (two prior): the weariness complication rides.
        assert!(
            active_complication(&sim, contagion).is_some_and(|c| c.id == comp.id),
            "by the third outbreak the ship's patience has worn through"
        );
        // And it shows in the description the player sees.
        assert_ne!(
            shown_description(&sim, contagion),
            contagion.description,
            "the escalation is visible before the choice"
        );
    }

    #[test]
    fn a_gated_outcome_is_offered_only_to_a_ship_that_earned_it() {
        // Content-depth event families round 12: state-gated outcomes. A crisis
        // offers a better exit only to a prepared ship — a fix a kept-expert bay
        // can attempt, a repair a banked reserve can buy — while the base choices
        // always show and the auto-resolve index-0 contract is untouched.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 43, &picks);

        // A knowledge floor: the coolant breach's master cooldown appears only
        // while the engineering bay's expertise is kept high.
        let breach = data.events.get("coolant_breach").unwrap();
        let master = breach
            .outcomes
            .iter()
            .position(|o| o.id == "master_controlled_cooldown")
            .unwrap();
        assert!(
            master > 0,
            "the gated outcome is authored after the base ones"
        );
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.4;
        assert!(
            !available_outcome_indices(&sim, breach).contains(&master),
            "a bay that has lost its masters cannot offer the master fix"
        );
        // Base outcomes are always on the table.
        assert!(available_outcome_indices(&sim, breach).contains(&0));
        sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.8;
        assert!(
            available_outcome_indices(&sim, breach).contains(&master),
            "expertise kept sharp unlocks the clean fix"
        );
        // …and it resolves by its real index like any outcome.
        apply_outcome(&mut sim, &data, breach, master);

        // A consequence gate: the hull fracture's shipyard repair appears only for
        // a ship that banked the war chest (ties back to the-full-coffers, it75).
        let fracture = data.events.get("hull_fracture").unwrap();
        let repair = fracture
            .outcomes
            .iter()
            .position(|o| o.id == "draw_on_the_war_chest")
            .unwrap();
        assert!(
            !available_outcome_indices(&sim, fracture).contains(&repair),
            "a ship with no reserve cannot draw on one"
        );
        sim.consequences.push("war_chest".to_string());
        assert!(
            available_outcome_indices(&sim, fracture).contains(&repair),
            "the banked reserve unlocks the proper repair years later"
        );
    }

    #[test]
    fn a_choice_targeting_complication_punishes_only_the_choice_it_names() {
        // Content-depth event families round 14: outcome-conditional complications.
        // The hull fracture's deferral twist rides on a ship that already puts work
        // off, but its extra toll lands only on the choice to defer *again* — fixing
        // the crack (or paying for a proper repair) escapes it.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let event = data.events.get("hull_fracture").unwrap();
        let comp = event
            .complications
            .iter()
            .find(|c| c.applies_to_outcomes.iter().any(|o| o == "monitor_it"))
            .expect("hull_fracture carries a choice-targeting complication");
        let defer = event
            .outcomes
            .iter()
            .position(|o| o.id == "monitor_it")
            .unwrap();
        let fix = event
            .outcomes
            .iter()
            .position(|o| o.id == "reinforce_now")
            .unwrap();

        // The twist rides only on a ship that already carries deferred work.
        let mut deferring = SimState::new_campaign(&data, "preservers", 67, &picks);
        deferring
            .consequences
            .push("deferred_maintenance".to_string());
        assert!(
            active_complication(&deferring, event).is_some_and(|c| c.id == comp.id),
            "the deferral twist rides on a ship that already defers"
        );

        // Hull change from applying an outcome, with or without the deferral history.
        let hull_delta = |outcome: usize, deferred: bool| -> f32 {
            let mut sim = SimState::new_campaign(&data, "preservers", 67, &picks);
            sim.resources.minerals = 100_000; // afford the reinforce
            if deferred {
                sim.consequences.push("deferred_maintenance".to_string());
            }
            let h0 = sim.ship.hull_integrity;
            apply_outcome(&mut sim, &data, event, outcome);
            sim.ship.hull_integrity - h0
        };

        // Deferring *again* on a deferring ship costs extra hull; fixing it does not.
        assert!(
            hull_delta(defer, true) < hull_delta(defer, false),
            "deferring again eats the complication's extra toll"
        );
        assert!(
            (hull_delta(fix, true) - hull_delta(fix, false)).abs() < 1e-6,
            "fixing the crack is untouched — the twist targets only the defer choice"
        );
    }

    #[test]
    fn a_complication_rides_only_when_its_state_gate_holds_and_lands_extra_toll() {
        // Content-depth event families round 6: system_failure carries a
        // complication that rides only while the engineering bay is itself
        // failing. When it rides it (a) shows in the description and (b) lands an
        // extra toll on top of whichever outcome was taken.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let template = data.events.get("system_failure").unwrap();
        assert!(!template.complications.is_empty());

        // Sound bay: no complication rides; the description is the plain one.
        let mut sound = SimState::new_campaign(&data, "adaptors", 51, &picks);
        sound
            .subsystems
            .get_mut("engineering_bay")
            .unwrap()
            .condition = 0.9;
        assert!(active_complication(&sound, template).is_none());
        assert_eq!(shown_description(&sound, template), template.description);

        // Failing bay: the complication rides, and its twist joins the shown text.
        let mut failing = SimState::new_campaign(&data, "adaptors", 51, &picks);
        failing
            .subsystems
            .get_mut("engineering_bay")
            .unwrap()
            .condition = 0.2;
        assert!(active_complication(&failing, template).is_some());
        assert!(shown_description(&failing, template).len() > template.description.len());

        // Same outcome, two states: the complicated run takes the heavier hull hit.
        let hull_of = |mut sim: SimState| {
            apply_outcome(&mut sim, &data, template, 0); // emergency_repair
            sim.ship.hull_integrity
        };
        let (a, b) = (sound.clone(), failing.clone());
        assert!(
            hull_of(b) < hull_of(a),
            "the complication lands an extra toll the flat event does not"
        );
    }

    #[test]
    fn the_triage_rule_pays_off_generations_after_it_is_written() {
        // Content-depth event-families round 5: a chain payoff completing a
        // formerly-dangling consequence. The cold triage rule (set by
        // `triage_rule`) re-fires as `the_rule_comes_due` only once that choice
        // was made — and its two ways out genuinely diverge (honor the cold law
        // vs break it, opposite morale/stability swings).
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 83, &picks);
        let payoff = data.events.get("the_rule_comes_due").unwrap();
        assert_eq!(
            payoff.requires_consequence,
            vec!["cold_triage_rule".to_string()]
        );
        sim.dynasty.generation = 4; // clear min_generation

        // Without the setup choice on record, the payoff stays out of the pool.
        assert!(
            !passes_gate(&sim, payoff),
            "the reckoning cannot fire before the cold rule was ever written"
        );
        // Writing the cold rule (the setup's consequence) unlocks it.
        sim.consequences.push("cold_triage_rule".to_string());
        assert!(passes_gate(&sim, payoff), "the written rule comes due");

        // The two resolutions move morale in opposite directions.
        let mut honor = sim.clone();
        let apply = payoff
            .outcomes
            .iter()
            .position(|o| o.id == "apply_the_rule")
            .unwrap();
        apply_outcome(&mut honor, &data, payoff, apply);
        let mut refuse = sim.clone();
        let brk = payoff
            .outcomes
            .iter()
            .position(|o| o.id == "break_the_rule")
            .unwrap();
        apply_outcome(&mut refuse, &data, payoff, brk);
        assert!(
            refuse.population.morale > honor.population.morale,
            "breaking the cold law lifts morale where honoring it costs it"
        );
    }

    #[test]
    fn the_provisioners_debt_becomes_a_branching_generational_chain() {
        // Content-depth provisioning round 7: complete the dangling `owed_a_favor`
        // debt the fuel-bargain seeded. Generations on, the strangers collect
        // (`the_debt_called_in`); reneging seeds `broke_a_bargain`, which itself
        // re-fires as `the_marked_hull` a further stretch on — a real branching
        // arc, not a single flat payoff.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 51, &picks);

        let called_in = data.events.get("the_debt_called_in").unwrap();
        assert_eq!(
            called_in.requires_consequence,
            vec!["owed_a_favor".to_string()]
        );
        sim.dynasty.generation = 5; // clear min_generation

        // No debt on record → the collectors never come.
        assert!(
            !passes_gate(&sim, called_in),
            "no collector comes for a debt that was never taken"
        );
        sim.consequences.push("owed_a_favor".to_string());
        assert!(passes_gate(&sim, called_in), "the taken favor comes due");

        // Honoring the debt closes it clean and never marks the hull.
        let mut honor = sim.clone();
        let hon = called_in
            .outcomes
            .iter()
            .position(|o| o.id == "honor_the_debt")
            .unwrap();
        apply_outcome(&mut honor, &data, called_in, hon);
        assert!(
            !honor.consequences.contains(&"broke_a_bargain".to_string()),
            "keeping the founders' word does not brand the ship an oathbreaker"
        );

        // Reneging keeps resources but seeds the reputation consequence.
        let mut renege = sim.clone();
        let ren = called_in
            .outcomes
            .iter()
            .position(|o| o.id == "renege_the_debt")
            .unwrap();
        apply_outcome(&mut renege, &data, called_in, ren);
        assert!(
            renege.consequences.contains(&"broke_a_bargain".to_string()),
            "disowning the debt marks the hull"
        );

        // The mark re-fires generations later, and only for a ship that reneged.
        let marked = data.events.get("the_marked_hull").unwrap();
        assert_eq!(
            marked.requires_consequence,
            vec!["broke_a_bargain".to_string()]
        );
        renege.dynasty.generation = 7; // clear the marked hull's later gate
        assert!(
            passes_gate(&renege, marked),
            "the closed ports find the ship that broke its word"
        );
        honor.dynasty.generation = 7;
        assert!(
            !passes_gate(&honor, marked),
            "a ship that kept its word is never turned away"
        );
    }

    #[test]
    fn the_tempting_world_trades_food_for_a_biocontamination_risk() {
        // Content-depth provisioning round 6: a garden-stop archetype the set
        // lacked — resupply from a living world, but the harvest can bring
        // something aboard. Gated on a real food shortage; the "land" choice
        // gains food yet dents BOTH agriculture and the medical bay (the
        // contaminant), where the sterile skim is safe but leaner.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "wanderers", 45, &picks);
        let event = data.events.get("the_tempting_world").unwrap();
        let famine = event.food_below.unwrap();
        // Put the ship on a phase it accepts, and hungry enough to be tempted.
        let template = data.contracts.get("seedfall").unwrap();
        let mut active = crate::simulation::contract::start_contract(template, &sim);
        active.phase = crate::data::contracts::ContractPhase::Travel;
        sim.contract = Some(active);

        sim.resources.food = famine + 2000;
        assert!(!passes_gate(&sim, event), "a full larder is not tempted");
        sim.resources.food = famine - 1;
        assert!(
            passes_gate(&sim, event),
            "a hungry ship meets the tempting world"
        );

        let land = event
            .outcomes
            .iter()
            .position(|o| o.id == "land_and_harvest")
            .unwrap();
        let (food0, agri0, med0) = (
            sim.resources.food,
            sim.subsystems["agriculture"].condition,
            sim.subsystems["medical_bay"].condition,
        );
        apply_outcome(&mut sim, &data, event, land);
        assert!(sim.resources.food > food0, "the harvest fills the holds");
        assert!(
            sim.subsystems["agriculture"].condition < agri0
                && sim.subsystems["medical_bay"].condition < med0,
            "the contaminant rides up into both the grow-decks and the wards"
        );
    }

    #[test]
    fn the_deep_stores_reward_foresight_only_when_a_famine_comes() {
        // Content-depth provisioning round 5: the insurance chain, the positive
        // mirror of the shortcut chains. The payoff (the_vaults_answer) needs
        // BOTH the early investment on record AND a famine now — foresight that
        // sits idle until the year it is everything.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 63, &picks);
        let payoff = data.events.get("the_vaults_answer").unwrap();
        assert_eq!(
            payoff.requires_consequence,
            vec!["deep_stores_built".to_string()]
        );
        assert!(payoff.food_below.is_some());
        let famine = payoff.food_below.unwrap();
        sim.dynasty.generation = 5; // clear min_generation

        // Vaults built but larder full → the payoff waits (insurance unspent).
        sim.consequences.push("deep_stores_built".to_string());
        sim.resources.food = famine + 5000;
        assert!(
            !passes_gate(&sim, payoff),
            "a stocked ship does not open its emergency vaults"
        );
        // Famine but no vaults ever built → nothing to open.
        let mut no_vaults = SimState::new_campaign(&data, "adaptors", 63, &picks);
        no_vaults.dynasty.generation = 5;
        no_vaults.resources.food = famine - 1;
        assert!(
            !passes_gate(&no_vaults, payoff),
            "with no vaults built, the foresight payoff cannot fire"
        );
        // Both: the vaults answer the famine.
        sim.resources.food = famine - 1;
        assert!(
            passes_gate(&sim, payoff),
            "built stores + a famine finally open the vaults"
        );
        // …and opening them actually relieves the hunger.
        let before = sim.resources.food;
        let open = payoff
            .outcomes
            .iter()
            .position(|o| o.id == "open_the_vaults")
            .unwrap();
        apply_outcome(&mut sim, &data, payoff, open);
        assert!(
            sim.resources.food > before,
            "opening the deep vaults feeds the ship"
        );
    }

    #[test]
    fn the_castaways_can_grow_the_ship_at_a_provisioning_cost() {
        // Content-depth provisioning round 4: the population-gain opportunity —
        // every prior provisioning beat shed people; this one can take them ON,
        // trading berths for stores. The two choices genuinely diverge: aboard
        // grows the crew and spends food; stores-only shrinks nothing and banks
        // food. Locks the new provisioning→population coupling.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let base = SimState::new_campaign(&data, "adaptors", 71, &picks);
        let event = data.events.get("the_castaways").unwrap();

        let mut aboard = base.clone();
        let take = event
            .outcomes
            .iter()
            .position(|o| o.id == "take_them_aboard")
            .unwrap();
        apply_outcome(&mut aboard, &data, event, take);

        let mut trade = base.clone();
        let stores = event
            .outcomes
            .iter()
            .position(|o| o.id == "take_the_stores_only")
            .unwrap();
        apply_outcome(&mut trade, &data, event, stores);

        assert!(
            aboard.population.count > base.population.count,
            "taking the castaways aboard grows the ship"
        );
        assert!(
            aboard.resources.food < trade.resources.food,
            "the berths cost food the stores-only trade instead banks"
        );
        assert_eq!(
            trade.population.count, base.population.count,
            "trading for stores adds no mouths"
        );
    }

    #[test]
    fn a_shortage_gate_holds_an_opportunity_until_the_ship_runs_low() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 13, &picks);
        // `the_dry_tank` only calls when the fuel fraction is at or below 0.2.
        let event = data.events.get("the_dry_tank").unwrap();
        assert_eq!(event.fuel_below, Some(0.2));
        // Put it in a phase it accepts.
        let template = data.contracts.get("deep_vein_survey").unwrap();
        let mut active = crate::simulation::contract::start_contract(template, &sim);
        active.phase = crate::data::contracts::ContractPhase::Travel;
        sim.contract = Some(active);

        sim.ship.fuel = 0.8;
        assert!(
            !passes_gate(&sim, event),
            "a full tank keeps the crisis away"
        );
        sim.ship.fuel = 0.1;
        assert!(passes_gate(&sim, event), "a near-dry tank surfaces it");
    }

    #[test]
    fn a_double_shortage_gate_needs_both_shortages_at_once() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 29, &picks);
        // `the_long_winter` gates on low food AND low energy together.
        let event = data.events.get("the_long_winter").unwrap();
        assert!(event.food_below.is_some() && event.energy_below.is_some());
        let (food_t, energy_t) = (event.food_below.unwrap(), event.energy_below.unwrap());

        // Only one shortage → still out of the pool.
        sim.resources.food = food_t - 1;
        sim.resources.energy = energy_t + 1000;
        assert!(
            !passes_gate(&sim, event),
            "low food alone is not the long winter"
        );
        sim.resources.food = food_t + 1000;
        sim.resources.energy = energy_t - 1;
        assert!(
            !passes_gate(&sim, event),
            "low energy alone is not the long winter"
        );
        // Both short → it fires.
        sim.resources.food = food_t - 1;
        sim.resources.energy = energy_t - 1;
        assert!(
            passes_gate(&sim, event),
            "hunger and cold together bring it"
        );
    }

    #[test]
    fn a_forbidden_consequence_closes_a_door_a_choice_slammed() {
        // Content-depth event families round 13: the negative gate. A generally
        // available opportunity is barred once a disqualifying history is on record
        // — trust never extended to a ship known to have broken its word — and a
        // multi-tag bar closes on *either* tag.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 57, &picks);

        // The stranger's trust is offered to a ship with a clean name…
        let trust = data.events.get("the_strangers_trust").unwrap();
        assert_eq!(
            trust.forbidden_consequence,
            vec!["broke_a_bargain".to_string()]
        );
        assert!(
            passes_gate(&sim, trust),
            "an unspoiled name is extended the stranger's trust"
        );
        // …and never again once the ship has broken a bargain.
        sim.consequences.push("broke_a_bargain".to_string());
        assert!(
            !passes_gate(&sim, trust),
            "a known oathbreaker is offered no trust"
        );

        // A multi-tag bar: the founders' vindication is closed by either a buried
        // record or a lost archive — you cannot revere a founding truth you hid or
        // forgot.
        let vindication = data.events.get("the_founders_vindicated").unwrap();
        assert!(vindication.forbidden_consequence.len() >= 2);
        let mut clean = SimState::new_campaign(&data, "preservers", 58, &picks);
        clean.dynasty.generation = 6;
        assert!(
            passes_gate(&clean, vindication),
            "an intact founding record can be vindicated"
        );
        clean.consequences.push("the_lost_archive".to_string());
        assert!(
            !passes_gate(&clean, vindication),
            "a ship that let its archive die cannot vindicate a founding it forgot"
        );
    }

    #[test]
    fn a_chronic_scarcity_gate_waits_for_a_lean_generation() {
        // Content-depth provisioning round 13: the persistence gate. `the_long_hunger`
        // tells a chronic hunger from one bad winter — it needs both a currently lean
        // larder *and* a shortage that has ground on for years, so a ship one season
        // into a famine does not yet face the long-hunger reckoning.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 63, &picks);
        let event = data.events.get("the_long_hunger").unwrap();
        let famine = event.food_below.expect("gates on a lean larder");
        let years = event.min_lean_food_years;
        assert!(years > 0, "the long hunger gates on a sustained shortage");

        // Lean today, but only just: no long-hunger reckoning yet.
        sim.resources.food = famine - 1;
        sim.lean_food_years = years - 1;
        assert!(
            !passes_gate(&sim, event),
            "one season of hunger is not yet a lean generation"
        );
        // A shortage that has ground on for years, still lean: it surfaces.
        sim.lean_food_years = years;
        assert!(
            passes_gate(&sim, event),
            "years of grinding scarcity bring the long hunger"
        );
        // A ship that has recovered its stores does not face it, however long the
        // past lean lasted (the streak resets on recovery in the tick).
        sim.resources.food = famine + 5000;
        assert!(
            !passes_gate(&sim, event),
            "a recovered larder ends the long hunger"
        );
    }

    #[test]
    fn a_paradox_gate_needs_abundance_and_scarcity_at_once() {
        // Content-depth provisioning round 12: the abundance gates (it75) gain their
        // first interaction with the shortage set. `the_gilded_hunger` surfaces only
        // when the ship is *both* rich in credits and starving — a fortune it cannot
        // eat — so neither condition alone brings it.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 53, &picks);
        let event = data.events.get("the_gilded_hunger").unwrap();
        let rich = event.credits_above.expect("gates on a fat treasury");
        let starving = event.food_below.expect("gates on an empty larder");

        // Rich but fed: no paradox.
        sim.resources.credits = rich + 1;
        sim.resources.food = starving + 1000;
        assert!(
            !passes_gate(&sim, event),
            "a rich, fed ship has no gilded hunger"
        );
        // Starving but poor: the ordinary famine, not this one.
        sim.resources.credits = rich - 1000;
        sim.resources.food = starving - 1;
        assert!(
            !passes_gate(&sim, event),
            "a poor, starving ship faces plain famine, not gilded hunger"
        );
        // Rich *and* starving: the fortune it cannot eat.
        sim.resources.credits = rich + 1;
        sim.resources.food = starving - 1;
        assert!(
            passes_gate(&sim, event),
            "wealth it cannot eat and a larder run dry, at once"
        );
    }

    #[test]
    fn a_founder_authority_gate_waits_for_a_lapsed_covenant() {
        // Content-depth campaign-skeleton round 14: the honest gate for covenant-lapse
        // content. `the_lapsed_covenant` stays out of the pool on a still-devoted
        // ship and surfaces only once loyalty to the founders has fallen far enough.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 60, &picks);
        let event = data.events.get("the_lapsed_covenant").unwrap();
        let ceiling = event.max_legacy_loyalty;
        assert!(ceiling > 0.0, "the covenant lapse gates on fallen loyalty");

        sim.population.legacy_loyalty = ceiling + 0.1;
        assert!(
            !passes_gate(&sim, event),
            "a still-devoted ship holds the founders' charter binding"
        );
        sim.population.legacy_loyalty = ceiling;
        assert!(
            passes_gate(&sim, event),
            "a lapsed covenant surfaces the reckoning"
        );
    }

    #[test]
    fn a_cohesion_gate_waits_for_a_reunited_ship() {
        // Content-depth campaign-skeleton round 13: the honest gate for recovery
        // content, the cohesion twin of min_morale. `the_mending` stays out of the
        // pool on a fracturing ship and surfaces only once unity has climbed back.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 59, &picks);
        let event = data.events.get("the_mending").unwrap();
        let floor = event.min_unity;
        assert!(floor > 0.0, "the mending gates on recovered cohesion");

        sim.population.unity = floor - 0.1;
        assert!(
            !passes_gate(&sim, event),
            "a fracturing ship has no mending to reflect on"
        );
        sim.population.unity = floor;
        assert!(
            passes_gate(&sim, event),
            "a reunited ship surfaces the mending"
        );
    }

    #[test]
    fn a_depopulation_gate_waits_for_a_thinned_crew() {
        // Content-depth campaign-skeleton round 12: the honest gate for crew-thinning
        // content, the descending mirror of min_morale. `the_thinning_decks` stays
        // out of the pool on a full ship and surfaces only once the crew has fallen
        // to or below its headcount ceiling.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 51, &picks);
        let event = data.events.get("the_thinning_decks").unwrap();
        let ceiling = event.max_population;
        assert!(ceiling > 0, "the thinning content gates on a headcount");

        sim.population.count = ceiling + 1;
        assert!(
            !passes_gate(&sim, event),
            "a full ship does not reckon with empty decks"
        );
        sim.population.count = ceiling;
        assert!(
            passes_gate(&sim, event),
            "a crew fallen to the ceiling surfaces the thinning"
        );
    }

    #[test]
    fn an_abundance_gate_waits_for_real_plenty_and_softness_worsens_the_winter() {
        // Content-depth provisioning round 11: the first gate keyed to *plenty*
        // rather than want. `the_fat_years` stays out of the pool at ordinary
        // stores and only surfaces when the granaries are genuinely swollen — and
        // feasting through it (grown_soft) makes the later long winter bite harder.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 41, &picks);
        let fat = data.events.get("the_fat_years").unwrap();
        let threshold = fat.food_above.expect("the fat years gate on abundance");

        // Ordinary and even lean stores: no fat-years choice.
        sim.resources.food = threshold - 1;
        assert!(
            !passes_gate(&sim, fat),
            "a merely comfortable ship has no surplus to reckon with"
        );
        // Granaries swollen past the threshold: the choice of plenty arrives.
        sim.resources.food = threshold + 1;
        assert!(
            passes_gate(&sim, fat),
            "genuine abundance surfaces the fat-years choice"
        );

        // The loop closes on the long winter: a ship that grew soft in the fat
        // years carries the soft-generation complication where a thrifty one does
        // not — the abundance choice reaches forward into the later famine.
        let winter = data.events.get("the_long_winter").unwrap();
        let soft = winter
            .complications
            .iter()
            .find(|c| c.requires_consequence.iter().any(|s| s == "grown_soft"))
            .expect("the long winter carries the soft-generation complication");
        assert!(
            active_complication(&sim, winter).is_none(),
            "a ship that never feasted meets the winter with its thrift intact"
        );
        // Feast through the fat years, then face the winter.
        let live_well = fat
            .outcomes
            .iter()
            .position(|o| o.long_term_consequences.iter().any(|s| s == "grown_soft"))
            .unwrap();
        apply_outcome(&mut sim, &data, fat, live_well);
        assert!(
            sim.consequences.iter().any(|c| c == "grown_soft"),
            "living well in the fat years softens the ship"
        );
        assert!(
            active_complication(&sim, winter).is_some_and(|c| c.id == soft.id),
            "the softened generation bears the long winter worse"
        );
    }

    #[test]
    fn a_condition_gate_waits_for_a_module_to_break_down() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 23, &picks);
        // `the_failing_air` only fires as the habitat plant physically fails.
        let event = data.events.get("the_failing_air").unwrap();
        assert_eq!(event.condition_below[0].id, "life_support_habitat");

        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 0.9;
        assert!(
            !passes_gate(&sim, event),
            "a sound plant keeps the crisis away"
        );
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 0.2;
        assert!(passes_gate(&sim, event), "a failing plant surfaces it");
    }

    #[test]
    fn an_era_ceiling_retires_deep_middle_content_before_homecoming() {
        // Content-depth campaign-skeleton round 4: the max_generation ceiling is
        // the mirror of min_generation — a deep-middle beat unlocks after the
        // founding generations and retires before the homecoming ones, so "the
        // ship is the only world" cannot fire once the ship is nearly home.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 61, &picks);
        let event = data.events.get("the_only_world").unwrap();
        assert!(event.min_generation > 0 && event.max_generation >= event.min_generation);

        // Before its era: still gated out by min_generation.
        sim.dynasty.generation = event.min_generation - 1;
        assert!(
            !passes_gate(&sim, event),
            "too early: the founders still live"
        );
        // Inside its era: it fires.
        sim.dynasty.generation = event.min_generation;
        assert!(passes_gate(&sim, event), "the deep middle surfaces it");
        // Past its era: the ceiling retires it.
        sim.dynasty.generation = event.max_generation + 1;
        assert!(
            !passes_gate(&sim, event),
            "too late: near home it is no longer the only world"
        );
    }

    #[test]
    fn a_neglected_reactor_blooms_into_a_medical_crisis_a_generation_later() {
        // Content-depth subsystems round 6: a cross-subsystem cascade *chain*.
        // Running the reactor hot (engineering neglect) records `reactor_run_hot`;
        // a generation on it re-fires as a radiation bloom in the medical bay —
        // engineering→medical coupling spread across time, not one event.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 29, &picks);

        // The creep gates on a worn engineering bay; running it hot records the tag.
        let creep = data.events.get("the_reactor_creep").unwrap();
        assert_eq!(creep.condition_below[0].id, "engineering_bay");
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.4;
        assert!(passes_gate(&sim, creep), "a worn bay surfaces the creep");
        let hot = creep
            .outcomes
            .iter()
            .position(|o| o.id == "run_it_hot")
            .unwrap();
        apply_outcome(&mut sim, &data, creep, hot);
        assert!(sim.consequences.contains(&"reactor_run_hot".to_string()));

        // The bloom waits on that neglect *and* a later generation.
        let bloom = data.events.get("the_radiation_bloom").unwrap();
        assert_eq!(
            bloom.requires_consequence,
            vec!["reactor_run_hot".to_string()]
        );
        sim.dynasty.generation = bloom.min_generation.saturating_sub(1);
        assert!(
            !passes_gate(&sim, bloom),
            "too soon: the bill is not yet due"
        );
        sim.dynasty.generation = bloom.min_generation;
        assert!(
            passes_gate(&sim, bloom),
            "a generation on, the reactor's debt blooms"
        );

        // Relining the shielding at the setup instead never records the debt.
        let mut prudent = SimState::new_campaign(&data, "adaptors", 29, &picks);
        prudent
            .subsystems
            .get_mut("engineering_bay")
            .unwrap()
            .condition = 0.4;
        let reline = creep
            .outcomes
            .iter()
            .position(|o| o.id == "reline_the_shielding")
            .unwrap();
        apply_outcome(&mut prudent, &data, creep, reline);
        prudent.dynasty.generation = bloom.min_generation;
        assert!(
            !passes_gate(&prudent, bloom),
            "a ship that paid for the shielding never sees the bloom"
        );
    }

    #[test]
    fn a_broken_garden_breakdown_couples_agriculture_to_the_medical_bay() {
        // Content-depth subsystems round 4: the agriculture breakdown gates on a
        // physically failing grow-deck, and its "fall back to soil" outcome is a
        // data-expressed cross-coupling — the lean years dent BOTH agriculture
        // and the medical bay (malnutrition load), the doc's canonical example.
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 37, &picks);
        let event = data.events.get("the_broken_beds").unwrap();
        assert_eq!(event.condition_below[0].id, "agriculture");

        // A sound garden keeps it away; a failing one surfaces it.
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.9;
        assert!(!passes_gate(&sim, event), "a sound garden keeps it away");
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.2;
        assert!(passes_gate(&sim, event), "a failing garden surfaces it");

        // The soil-farming fall-back touches two subsystems at once.
        let soil = event
            .outcomes
            .iter()
            .position(|o| o.id == "fall_back_to_soil")
            .expect("the broken beds can fall back to soil");
        let med_before = sim.subsystems["medical_bay"].condition;
        apply_outcome(&mut sim, &data, event, soil);
        assert!(
            sim.subsystems["medical_bay"].condition < med_before,
            "the lean years load the medical bay, not just the gardens"
        );
    }

    #[test]
    fn an_energy_shortage_gate_waits_for_a_browning_reactor() {
        let data = GameData::load().unwrap();
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "adaptors", 17, &picks);
        // `the_dimming` only enters the pool when energy is at or below 1200.
        let event = data.events.get("the_dimming").unwrap();
        assert_eq!(event.energy_below, Some(1200));

        sim.resources.energy = 5000;
        assert!(
            !passes_gate(&sim, event),
            "a full grid keeps the crisis away"
        );
        sim.resources.energy = 800;
        assert!(passes_gate(&sim, event), "a browning grid surfaces it");
    }

    #[test]
    fn event_chance_is_capped() {
        let data = GameData::load().unwrap();
        assert!((event_chance(&data.config, 100, 1.0) - data.config.event_chance_cap).abs() < 1e-6);
        assert!((event_chance(&data.config, 0, 0.0) - data.config.event_chance_base).abs() < 1e-6);
    }

    #[test]
    fn starving_ship_doubles_food_weight_in_scoring() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.events.get("population_growth").unwrap();
        let feed = &template.outcomes[0]; // food -300
        let hold = &template.outcomes[1]; // no food cost

        sim.resources.food = 100; // below low_food_threshold
        let feed_starving = score_outcome(feed, &sim, &data.config);
        sim.resources.food = 5000;
        let feed_fed = score_outcome(feed, &sim, &data.config);
        assert!(
            feed_starving < feed_fed,
            "spending food must score worse while starving"
        );

        sim.resources.food = 100;
        assert!(score_outcome(hold, &sim, &data.config) > score_outcome(feed, &sim, &data.config));
    }

    #[test]
    fn apply_outcome_clears_pending_and_records_consequences() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "adaptors",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.events.get("system_failure").unwrap().clone();
        sim.pending_event = Some(crate::state::sim::PendingEvent {
            template_id: template.id.clone(),
            rolled_month_clock: sim.month_clock,
        });

        apply_outcome(&mut sim, &data, &template, 1); // reroute_power
        assert!(sim.pending_event.is_none());
        assert!(sim
            .consequences
            .contains(&"deferred_maintenance".to_owned()));
        assert!(sim.ship.life_support < 1.0);
    }

    #[test]
    fn a_force_return_outcome_turns_the_ship_home() {
        use crate::data::contracts::ContractPhase;
        use crate::simulation::contract::{advance_contract, start_contract};

        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));

        // Put the ship on-station so there is a Return leg to jump to.
        loop {
            let p = advance_contract(&mut sim, &data.config, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }

        // The catastrophic reactor-scram outcome forces the mission home early.
        let scram = data.events.get("reactor_scram").unwrap().clone();
        let idx = scram
            .outcomes
            .iter()
            .position(|o| o.force_return)
            .expect("reactor_scram carries a force_return outcome");
        apply_outcome(&mut sim, &data, &scram, idx);

        assert_eq!(
            sim.contract.as_ref().unwrap().phase,
            ContractPhase::Return,
            "a force_return outcome jumps the contract onto its return leg"
        );
    }
}
