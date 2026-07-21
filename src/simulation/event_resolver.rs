//! Event rolling, outcome scoring, and resolution (GDD §5.4).

pub mod skeleton;

use crate::data::events::{EventCategory, EventOutcome, EventTemplate};
use crate::data::{GameConfig, GameData};
use crate::simulation::subsystems;
use crate::state::sim::{PendingEvent, SimState};

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
    // Era ceilings (content-depth round 4): 0 = ungated, else the event has
    // passed out of its era once the voyage is beyond the cap.
    if template.max_year != 0 && sim.year() > template.max_year {
        return false;
    }
    if template.max_generation != 0 && sim.dynasty.generation > template.max_generation {
        return false;
    }
    sim.year() >= template.min_year
        && sim.dynasty.generation >= template.min_generation
        && sim.population.cultural_drift >= template.min_cultural_drift
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
    // …or wound / mend / re-teach a subsystem (content-depth coupling): an
    // engineering crisis damages the engineering bay, a teaching succession
    // restores its lost know-how. Unknown ids are ignored.
    for delta in &outcome.subsystem_deltas {
        if let Some(state) = sim.subsystems.get_mut(&delta.id) {
            state.condition = (state.condition + delta.condition).clamp(0.0, 1.0);
            state.knowledge = (state.knowledge + delta.knowledge).clamp(0.0, 1.0);
        }
    }
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
