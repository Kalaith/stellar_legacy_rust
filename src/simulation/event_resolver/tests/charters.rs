//! Where the ship is colors what it meets: charter tags, destination
//! pools, route hazard, and the corps that quiets what a route breeds.

use super::*;

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
fn a_cryo_ark_crisis_fires_only_on_the_ark_run() {
    // Content-depth provisioning round 23: the ark run's signature content, gated to
    // its `cryo_ark` charter tag. The Failing Bank cannot surface on an ordinary
    // mining charter, only on the sleeper-ark in transit.
    use crate::data::contracts::ContractPhase;
    use crate::simulation::contract::start_contract;
    let data = GameData::load().unwrap();
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 61, &picks);
    let tmpl = data.events.get("the_failing_bank").unwrap();

    // An ordinary mining charter, in transit: no cryo-ark tag, so it is barred.
    let mut ordinary = start_contract(
        &data.contracts.get("deep_vein_survey").unwrap().clone(),
        &sim,
    );
    ordinary.phase = ContractPhase::Travel;
    sim.contract = Some(ordinary);
    assert!(
        !passes_gate(&sim, tmpl),
        "the failing bank does not surface on an ordinary run"
    );

    // The ark run, in transit: carries cryo_ark, so the crisis can fire.
    let mut ark = start_contract(&data.contracts.get("the_ark_run").unwrap().clone(), &sim);
    ark.phase = ContractPhase::Travel;
    sim.contract = Some(ark);
    assert!(
        passes_gate(&sim, tmpl),
        "the failing bank fires on the sleeper-ark in transit"
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
        category_weights(sim, &data)
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
fn a_well_armed_ship_deters_a_lawless_route() {
    // Content-depth charters round 27: the ship's combat loadout cuts into a charter's
    // route hazard — a well-armed ship makes scavengers and raiders keep their distance,
    // so the same dangerous writ breeds fewer crises on a gunship than on an unarmed hull,
    // and a heavily-armed ship can deter the whole of the route's added risk.
    let data = GameData::load().unwrap();
    let mit = data.config.ship.hazard_combat_mitigation;
    assert!(
        mit > 0.0,
        "this test needs the hazard-combat deterrence enabled"
    );
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 51, &picks);
    let dangerous = data.contracts.get("hollow_fleet").unwrap().clone();
    assert!(dangerous.hazard > 0.0, "a derelict field is a risk profile");
    sim.contract = Some(crate::simulation::contract::start_contract(
        &dangerous, &sim,
    ));

    let crisis_weight = |sim: &SimState| {
        category_weights(sim, &data)
            .iter()
            .find(|(c, _)| *c == EventCategory::ImmediateCrisis)
            .unwrap()
            .1
    };

    // Unarmed hull: the route's full hazard lands.
    sim.ship.weapon = None;
    let unarmed_w = crisis_weight(&sim);

    // A gun aboard: the route breeds fewer crises.
    sim.ship.weapon = Some("spinal_railgun".to_string());
    let combat = crate::simulation::ship::loadout_stats(&sim, &data).combat;
    assert!(combat > 0, "the railgun arms the ship");
    let armed_w = crisis_weight(&sim);
    assert!(
        armed_w < unarmed_w,
        "guns deter a lawless route: {armed_w} armed vs {unarmed_w} unarmed"
    );
    // The deterrence is exactly combat × mitigation, up to cancelling the whole hazard.
    let expected_drop = (combat as f32 * mit).min(dangerous.hazard);
    assert!(
        ((unarmed_w - armed_w) - expected_drop).abs() < 1e-5,
        "the crisis drop is combat × mitigation ({expected_drop})"
    );
}

#[test]
fn a_well_kept_security_corps_quiets_the_crises_a_route_breeds() {
    // Content-depth subsystems round 21: the corps' third domain — it defends the
    // ship against the crises a dangerous route and a distressed hull breed. A
    // sound corps dampens the immediate-crisis category weight; a wrecked one does
    // not; and even a perfect corps never dampens below the floor.
    let data = GameData::load().unwrap();
    assert!(
        data.config.subsystems.security_crisis_mitigation > 0.0,
        "this test needs the security crisis coupling enabled"
    );
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 12, &picks);

    let crisis_weight = |sim: &SimState| {
        category_weights(sim, &data)
            .iter()
            .find(|(c, _)| matches!(c, EventCategory::ImmediateCrisis))
            .map(|(_, w)| *w)
            .unwrap()
    };

    sim.subsystems.get_mut("security").unwrap().condition = 1.0;
    let sound = crisis_weight(&sim);
    sim.subsystems.get_mut("security").unwrap().condition = 0.1;
    let wrecked = crisis_weight(&sim);
    assert!(
        sound < wrecked,
        "a sound corps breeds fewer crises than a wrecked one: {sound} vs {wrecked}"
    );
    assert!(
        sound >= data.config.subsystems.crisis_weight_floor,
        "even a perfect corps never dampens the crisis weight below its floor"
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
