//! What a wound leaves behind: a scar written by one event that makes the
//! next one of its kind land harder.

use super::*;

#[test]
fn a_robbed_grave_answers_the_ships_own_ghost() {
    // Content-depth event families round 33: the `grave_robbed` consequence arc closes. Boarding
    // and stripping a derelict (the Silent Hull) records `grave_robbed`; years later the Ghost
    // Signal — the ship's own call sign out of a year unlived — gains a complication gated on
    // that deed, read by the crew who plundered the dead as the grave answering. So the crime
    // comes home on a ship that committed it and passes clean over one that spoke for its dead.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let template = data.events.get("ghost_signal").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| c.requires_consequence.contains(&"grave_robbed".to_string()))
        .expect("the ghost signal carries a robbed-grave reckoning");

    // A ship innocent of grave-robbing (and undrifted, so the omen twist stays out too) meets
    // the ghost as a puzzle, not a debt.
    let mut clean = SimState::new_campaign(&data, "preservers", 19, &picks);
    clean.population.cultural_drift = 0.0;
    assert!(
        active_complication(&clean, template).is_none(),
        "a ship that never robbed a grave hears no reckoning in the ghost signal"
    );

    // Record the deed the Silent Hull leaves: now the grave answers on the dead channel.
    clean.consequences.push("grave_robbed".to_string());
    assert_eq!(
        active_complication(&clean, template).map(|c| &c.id),
        Some(&comp.id),
        "the derelict the ship stripped answers its own ghost"
    );
    assert!(shown_description(&clean, template).contains("debt called in"));
}

#[test]
fn a_scarred_reactor_meets_its_next_scram_worse() {
    // Content-depth event families round 34: the `scarred_reactor` consequence arc. Hand-patching
    // a coolant breach or hot-restarting a scrammed core records `scarred_reactor`; a later
    // Reactor Scram then carries a complication read by the engineers who know the core's history
    // — the old hand-patches making a second hot restart bite harder, its extra toll targeted to
    // the hot-override choice that re-gambles the scarred core. A clean core meets the scram fresh.
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let template = data.events.get("reactor_scram").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| {
            c.requires_consequence
                .contains(&"scarred_reactor".to_string())
        })
        .expect("the reactor scram carries a scarred-core reckoning");
    assert!(
        comp.applies_to_outcomes
            .contains(&"hot_override".to_string()),
        "the scar's extra toll lands on the hot restart, not the careful cold one"
    );

    // A core never scarred meets the scram fresh — no reckoning.
    let mut clean = SimState::new_campaign(&data, "preservers", 21, &picks);
    assert!(
        active_complication(&clean, template).is_none(),
        "an unscarred core meets the scram fresh"
    );

    // Record the scar the coolant breach or a prior scram leaves: now the core remembers.
    clean.consequences.push("scarred_reactor".to_string());
    assert_eq!(
        active_complication(&clean, template).map(|c| &c.id),
        Some(&comp.id),
        "a core that has scarred before meets its next scram worse"
    );
    assert!(shown_description(&clean, template).contains("scrammed before"));
}

#[test]
fn a_stimulant_debt_comes_due_in_a_later_fever() {
    // Content-depth event families round 35: the `stimulant_debt` consequence arc. Pushing
    // through the Long Sleep on stimulants records `stimulant_debt`; a later Quiet Fever then
    // finds a crew hollowed by the old borrowed alertness, with no reserves to fight it — extra
    // dead, morale and stability sinking. A rested crew (no debt) meets the fever fresh. (The
    // debt complication is checked after the broken-ward and soft-years ones, so a sound
    // infirmary and a crew not long-soft isolate it.)
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let template = data.events.get("quiet_fever").unwrap();
    let comp = template
        .complications
        .iter()
        .find(|c| {
            c.requires_consequence
                .contains(&"stimulant_debt".to_string())
        })
        .expect("the quiet fever carries a stimulant-debt reckoning");

    // A rested crew: sound ward, not long-soft, no debt — no complication rides.
    let mut clean = SimState::new_campaign(&data, "preservers", 23, &picks);
    clean.subsystems.get_mut("medical_bay").unwrap().condition = 1.0;
    clean.fat_food_years = 0;
    assert!(
        active_complication(&clean, template).is_none(),
        "a rested crew with a sound ward meets the fever fresh"
    );

    // Record the debt the stimulant regime leaves: now it comes due.
    clean.consequences.push("stimulant_debt".to_string());
    assert_eq!(
        active_complication(&clean, template).map(|c| &c.id),
        Some(&comp.id),
        "the borrowed alertness comes due on a crew that spent its reserves"
    );
    assert!(shown_description(&clean, template).contains("borrowed against its own body"));
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
