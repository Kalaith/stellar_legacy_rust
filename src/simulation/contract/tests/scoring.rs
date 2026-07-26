//! What the voyage is worth when it ends: the success bands, the
//! milestone that pays once, and what a mission cut short still earns.

use super::*;

#[test]
fn score_bands_match_gdd_thresholds() {
    let full = vec![metric(1.0, 1.0, 1.0)];
    assert_eq!(score_success(&full).1, SuccessLevel::Complete);

    let partial = vec![metric(1.0, 1.0, 0.75)];
    assert_eq!(score_success(&partial).1, SuccessLevel::Partial);

    let pyrrhic = vec![metric(1.0, 1.0, 0.5)];
    assert_eq!(score_success(&pyrrhic).1, SuccessLevel::Pyrrhic);

    let failure = vec![metric(1.0, 1.0, 0.1)];
    assert_eq!(score_success(&failure).1, SuccessLevel::Failure);
}

#[test]
fn overshooting_a_target_does_not_overscore() {
    let metrics = vec![metric(0.5, 1.0, 3.0), metric(0.5, 1.0, 0.0)];
    let (score, _) = score_success(&metrics);
    assert!((score - 0.5).abs() < f32::EPSILON);
}

#[test]
fn objective_fraction_clamps_and_zero_target_is_complete() {
    let (_data, mut sim) = armed(3, "deep_vein_survey");
    let c = sim.contract.as_mut().unwrap();
    c.objective_progress = c.objective_target * 3.0;
    assert_eq!(c.objective_fraction(), 1.0, "overshoot clamps to full");
    c.objective_progress = 0.0;
    assert_eq!(c.objective_fraction(), 0.0);
    c.objective_target = 0.0;
    assert_eq!(c.objective_fraction(), 1.0, "a zero target counts as met");
}

#[test]
fn milestone_reward_lands_once_on_reach() {
    use crate::data::{GameData, ResourceDelta};
    use crate::state::sim::SimState;

    let data = GameData::load().unwrap();
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        31,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let mut contract = start_contract(data.contracts.get("deep_vein_survey").unwrap(), &sim);
    // Force the first milestone to fire immediately with a known reward.
    contract.milestones[0].progress_threshold = 0.0;
    contract.milestones[0].reached = false;
    contract.milestones[0].reward = ResourceDelta {
        minerals: 500,
        ..Default::default()
    };
    sim.contract = Some(contract);

    let before = sim.resources.minerals;
    advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    assert_eq!(
        sim.resources.minerals,
        before + 500,
        "the reward lands the year the milestone is reached"
    );

    let after = sim.resources.minerals;
    advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    assert_eq!(
        sim.resources.minerals, after,
        "an already-reached milestone does not pay out again"
    );
}

#[test]
fn a_truncated_mission_pays_proportional_to_the_objective() {
    let (data, mut sim) = armed(7, "deep_vein_survey");
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();

    // Make station, then bank a clean quarter of the objective.
    loop {
        let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
        if p.phase_changed == Some(ContractPhase::Operation) {
            break;
        }
    }
    {
        let c = sim.contract.as_mut().unwrap();
        c.objective_progress = c.objective_target * 0.25;
    }

    // Turn back mid-Operation and fly the return leg home.
    assert!(
        jump_to_return(&mut sim),
        "turning back mid-Operation is allowed"
    );
    assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Return);
    let total = sim.contract.as_ref().unwrap().total_months();
    while sim.contract.as_ref().unwrap().months_elapsed < total {
        advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    }

    let contract = sim.contract.as_ref().unwrap();
    assert_eq!(
        contract.objective_fraction(),
        0.25,
        "objective is frozen through Return"
    );
    let pay = prorated_reward(&template.reward, contract.objective_fraction());
    assert_eq!(pay.credits, template.reward.credits / 4);
    assert_eq!(pay.minerals, template.reward.minerals / 4);
    assert!(
        pay.credits > 0 && pay.credits < template.reward.credits,
        "prorated pay is neither full nor zero"
    );
}

#[test]
fn an_abort_in_travel_pays_nothing() {
    let (data, mut sim) = armed(4, "deep_vein_survey");
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();

    // A few months into Travel — no objective work has happened.
    for _ in 0..50 {
        advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    }
    assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Travel);

    assert!(jump_to_return(&mut sim));
    assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Return);
    let total = sim.contract.as_ref().unwrap().total_months();
    while sim.contract.as_ref().unwrap().months_elapsed < total {
        advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    }

    let contract = sim.contract.as_ref().unwrap();
    assert_eq!(
        contract.objective_fraction(),
        0.0,
        "no objective work → no pay"
    );
    let pay = prorated_reward(&template.reward, contract.objective_fraction());
    assert_eq!(pay.credits, 0);
    assert_eq!(pay.minerals, 0);
}

#[test]
fn resource_efficiency_tracks_lean_months_across_the_voyage() {
    let (data, mut sim) = armed(6, "deep_vein_survey");
    let efficiency = |sim: &crate::state::sim::SimState| {
        sim.contract
            .as_ref()
            .unwrap()
            .metrics
            .iter()
            .find(|m| m.kind == MetricKind::ResourceEfficiency)
            .unwrap()
            .current
    };

    // Ten well-provisioned months: full marks.
    sim.resources.food = data.config.low_food_threshold + 1_000;
    sim.resources.energy = data.config.low_energy_threshold + 1_000;
    for _ in 0..10 {
        advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    }
    assert_eq!(
        efficiency(&sim),
        1.0,
        "a voyage that never runs low scores full efficiency"
    );

    // Ten months with the larder empty: only the energy half banks credit,
    // so the running fraction settles at (10*2 + 10*1) / (20*2) = 0.75.
    sim.resources.food = 0;
    for _ in 0..10 {
        advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    }
    assert!(
        (efficiency(&sim) - 0.75).abs() < 1e-6,
        "lean months drag the voyage-long score: {}",
        efficiency(&sim)
    );

    // The lean stretch stays on the record after stores recover.
    sim.resources.food = data.config.low_food_threshold + 1_000;
    advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    assert!(
        efficiency(&sim) < 1.0,
        "a famine is not forgotten once the stores refill"
    );
}

#[test]
fn a_ships_name_bends_what_a_charter_pays() {
    // Content-depth charters round 29: a reputation the writ prizes lifts its pay, a
    // notorious one cuts it, a neutral name pays the base. The Sanctuary Run scales its pay
    // on mercy; an ordinary charter that names no trait ignores reputation entirely.
    let data = GameData::load().unwrap();
    let sanctuary = data.contracts.get("the_sanctuary_run").unwrap();
    assert!(
        !sanctuary.reward_reputation_trait.is_empty() && sanctuary.reward_reputation_scale > 0.0,
        "this test needs the reputation-reward coupling enabled"
    );
    let picks = crate::state::sim::founding_faction_ids(&data);
    let mut sim = SimState::new_campaign(&data, "preservers", 14, &picks);

    // A neutral name (the launch state): the base terms.
    assert!(
        (reputation_reward_multiplier(&sim, sanctuary) - 1.0).abs() < 1e-6,
        "a neutral name pays the base terms"
    );

    // A famously merciful ship: a premium above the base.
    sim.reputation.insert("mercy".to_string(), 1.0);
    let merciful = reputation_reward_multiplier(&sim, sanctuary);
    assert!(merciful > 1.0, "a merciful ship earns more on a relief run");

    // A merciless ship: a discount, but floored — a name never erases the pay.
    sim.reputation.insert("mercy".to_string(), 0.0);
    let merciless = reputation_reward_multiplier(&sim, sanctuary);
    assert!(
        (0.5..1.0).contains(&merciless),
        "a merciless ship earns less, but never nothing ({merciless})"
    );

    // An ordinary charter that names no trait ignores reputation entirely.
    let plain = data.contracts.get("deep_vein_survey").unwrap();
    assert_eq!(
        reputation_reward_multiplier(&sim, plain),
        1.0,
        "a charter that names no trait pays flat"
    );
}
