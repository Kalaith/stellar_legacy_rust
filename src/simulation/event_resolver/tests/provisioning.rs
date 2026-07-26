//! Hunger and the stores: the dilemmas a lean ship is offered and what
//! foresight buys when the winter finally comes.

use super::*;

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
