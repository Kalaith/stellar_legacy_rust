//! What makes the objective move: the phase the ship is in, the hull and
//! crew it brought, and the craft the work sharpens along the way.

use super::*;

#[test]
fn phases_are_set_from_the_authored_segments() {
    let (data, mut sim) = armed(1, "deep_vein_survey");

    // Month 1 crosses the pre-launch Preparation into Travel.
    let first = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    assert_eq!(first.phase_changed, Some(ContractPhase::Travel));

    // Travel holds until the authored travel years elapse, then Operation.
    let op_start = loop {
        let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
        if let Some(phase) = p.phase_changed {
            assert_eq!(
                phase,
                ContractPhase::Operation,
                "travel yields to operation"
            );
            break sim.contract.as_ref().unwrap().months_elapsed;
        }
        assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Travel);
    };
    // deep_vein_survey travels 110 years before making station.
    assert_eq!(op_start, 110 * 12 + 1);
}

#[test]
fn objective_accrues_only_during_operation() {
    let (data, mut sim) = armed(2, "deep_vein_survey");
    // Neutralize the round-22 crew-morale and round-34 crew-unity factors (as speed/combat are
    // passed 0) so this isolates the base accrual rate: at the 0.5 midpoint each factor is 1.0.
    sim.population.morale = 0.5;
    sim.population.unity = 0.5;

    // Nothing accrues in Preparation or Travel.
    loop {
        let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
        if p.phase_changed == Some(ContractPhase::Operation) {
            break;
        }
        assert_eq!(
            sim.contract.as_ref().unwrap().objective_progress,
            0.0,
            "no objective work before the ship is on-station"
        );
    }
    // The first on-station month accrues one base_rate share (speed 0).
    let c = sim.contract.as_ref().unwrap();
    let expected = c.objective_target / c.operation_months() as f32;
    assert!(
        (c.objective_progress - expected).abs() < 1e-3,
        "one operation month accrues base_rate: {} vs {expected}",
        c.objective_progress
    );
}

#[test]
fn a_bigger_hold_hauls_a_material_writ_faster_but_not_a_survey() {
    // Content-depth charters round 24: cargo capacity quickens a *haul* objective (a
    // mining run measured in tonnes) but is dead weight on a mission whose objective
    // is not a quantity of material.
    let first_operation_accrual = |contract_id: &str, cargo: i32| -> f32 {
        let (data, mut sim) = armed(9, contract_id);
        loop {
            let p = advance_contract(&mut sim, &data.config, 0, 0, cargo, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }
        sim.contract.as_ref().unwrap().objective_progress
    };

    // the_deep_camp sets objective_cargo_scaling > 0 — a big hold hauls more tonnage.
    let small_hold = first_operation_accrual("the_deep_camp", 0);
    let big_hold = first_operation_accrual("the_deep_camp", 400);
    assert!(
        big_hold > small_hold + 1e-3,
        "a haul writ accrues faster with a bigger hold: {big_hold} vs {small_hold}"
    );

    // deep_vein_survey (proof-of-yield cores) sets no cargo scaling — hold is dead weight.
    let survey_small = first_operation_accrual("deep_vein_survey", 0);
    let survey_big = first_operation_accrual("deep_vein_survey", 400);
    assert!(
        (survey_big - survey_small).abs() < 1e-4,
        "a survey is indifferent to hold size: {survey_big} vs {survey_small}"
    );
}

#[test]
fn a_roomy_hull_keeps_more_of_what_it_carries() {
    // Content-depth charters round 28: crew_capacity (berths) eases a preserve charter's
    // attrition — crew_capacity's first mechanical role. Two identical ark runs, one carried
    // by a cramped hull (crew 0) and one by a roomy one (crew 30), diverge in a single
    // voyage-month: the roomy ship loses fewer of its sleepers.
    let relief = {
        let (data, _sim) = armed(6, "the_ark_run");
        data.config.ship.preserve_berth_relief
    };
    assert!(
        relief > 0.0,
        "this test needs the berth-relief coupling enabled"
    );

    let loss = |crew: i32| -> f32 {
        let (data, mut sim) = armed(6, "the_ark_run");
        let before = sim.contract.as_ref().unwrap().objective_progress;
        advance_contract(&mut sim, &data.config, 0, 0, 0, crew);
        before - sim.contract.as_ref().unwrap().objective_progress
    };
    let cramped = loss(0); // no berths → the authored attrition, in full
    let roomy = loss(30); // ample berths → eased attrition
    assert!(cramped > 0.0, "a cramped hull still loses to the cold");
    assert!(
        roomy < cramped,
        "a roomy hull keeps more of its charge: {roomy} lost vs {cramped}"
    );
}

#[test]
fn a_preserve_charter_sets_out_full_and_only_erodes() {
    // Content-depth charters round 23: a fundamentally different objective shape. An
    // ark run carries its full objective from the start and loses a little every
    // voyage-month; it never accrues, where an ordinary charter builds from zero.
    let (data, mut sim) = armed(4, "the_ark_run");
    {
        let c = sim.contract.as_ref().unwrap();
        assert!(c.preserve_objective, "the ark run preserves its objective");
        assert_eq!(
            c.objective_progress, c.objective_target,
            "an ark run sets out carrying all its sleepers"
        );
    }

    // A voyage month erodes it (never accrues): each on-voyage step loses ground.
    let before = sim.contract.as_ref().unwrap().objective_progress;
    advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    let after = sim.contract.as_ref().unwrap().objective_progress;
    assert!(
        after < before,
        "the cold banks lose a little every month: {before} -> {after}"
    );
    assert_eq!(
        sim.contract.as_ref().unwrap().phase,
        ContractPhase::Travel,
        "the loss begins the moment the crossing does"
    );

    // Contrast: an ordinary charter starts empty and builds.
    let (_d, sim2) = armed(4, "deep_vein_survey");
    assert_eq!(
        sim2.contract.as_ref().unwrap().objective_progress,
        0.0,
        "an ordinary charter builds its objective from zero"
    );
}

#[test]
fn a_high_hearted_crew_works_the_objective_faster() {
    // Content-depth charters round 22: the mission's coupling to the crew's spirits.
    // A devoted crew drives the objective harder than a dispirited one, all else
    // equal — the crew-state twin of the loadout accrual levers.
    let first_operation_accrual = |morale: f32| -> f32 {
        let (data, mut sim) = armed(9, "deep_vein_survey");
        sim.population.morale = morale; // advance_contract never moves morale
        loop {
            let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }
        sim.contract.as_ref().unwrap().objective_progress
    };
    let devoted = first_operation_accrual(0.95);
    let broken = first_operation_accrual(0.15);
    assert!(
        devoted > broken * 1.05,
        "a high-hearted crew works the objective faster: {devoted} vs {broken}"
    );
}

#[test]
fn a_united_crew_works_the_objective_faster_than_a_fractured_one() {
    // Content-depth charters round 34: the mission's coupling to the crew's *cohesion*, the
    // second crew-state lever distinct from its spirits (round 22). A crew rowing as one works
    // the objective faster than a fractured one pulling different ways, all else equal (morale
    // is the same fresh-campaign value in both runs, so this isolates unity).
    let first_operation_accrual = |unity: f32| -> f32 {
        let (data, mut sim) = armed(9, "deep_vein_survey");
        sim.population.unity = unity; // advance_contract never moves unity
        loop {
            let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }
        sim.contract.as_ref().unwrap().objective_progress
    };
    let united = first_operation_accrual(0.95);
    let fractured = first_operation_accrual(0.15);
    assert!(
        united > fractured * 1.05,
        "a united crew works the objective faster than a fractured one: {united} vs {fractured}"
    );
}

#[test]
fn combat_quickens_a_contested_writ_but_not_a_quiet_one() {
    // Content-depth charters round 21: the ship's guns — until now good only for
    // a Wanderer's gamble — now work a *contested* mission's objective faster,
    // while a mission that rewards no firepower stays indifferent to how the ship
    // is armed. Fly each writ to its first on-station month at two loadouts and
    // compare that month's accrual.
    let first_operation_accrual = |contract_id: &str, combat: i32| -> f32 {
        let (data, mut sim) = armed(9, contract_id);
        loop {
            let p = advance_contract(&mut sim, &data.config, 0, combat, 0, 0);
            if p.phase_changed == Some(ContractPhase::Operation) {
                break;
            }
        }
        sim.contract.as_ref().unwrap().objective_progress
    };

    // warden_patrol sets objective_combat_scaling > 0 — a heavily armed ship
    // works the contested lane measurably faster than an unarmed one.
    let unarmed = first_operation_accrual("warden_patrol", 0);
    let heavily_armed = first_operation_accrual("warden_patrol", 8);
    assert!(
        heavily_armed > unarmed + 1e-3,
        "a contested writ accrues faster with guns: {heavily_armed} vs {unarmed}"
    );

    // deep_vein_survey sets no combat scaling — firepower is dead weight there,
    // so the same two loadouts accrue identically.
    let quiet_unarmed = first_operation_accrual("deep_vein_survey", 0);
    let quiet_armed = first_operation_accrual("deep_vein_survey", 8);
    assert!(
        (quiet_armed - quiet_unarmed).abs() < 1e-4,
        "a quiet survey is indifferent to arms: {quiet_armed} vs {quiet_unarmed}"
    );
}

#[test]
fn a_degraded_key_module_works_the_mission_slower() {
    // Content-depth subsystems round 14: the subsystem axis's first coupling to
    // the mission. The deep vein survey's work leans on the engineering bay; a
    // rotting bay mines slower than a pristine one, while a charter with no key
    // module is indifferent to any module's state.
    let data = GameData::load().unwrap();
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    assert_eq!(template.objective_subsystem, "engineering_bay");

    // Objective banked over one operation year at a given bay condition.
    let mined = |bay: f32| -> f32 {
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 73, &picks);
        sim.contract = Some(start_contract(&template, &sim));
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = bay;
        // Fast-forward the clock into the Operation window, then bank a year.
        let ops_start = template
            .phases
            .iter()
            .take_while(|p| p.kind != ContractPhase::Operation)
            .map(|p| p.years * 12)
            .sum::<u32>();
        sim.contract.as_mut().unwrap().months_elapsed = ops_start;
        let before = sim.contract.as_ref().unwrap().objective_progress;
        for _ in 0..12 {
            advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
        }
        sim.contract.as_ref().unwrap().objective_progress - before
    };

    let pristine = mined(1.0);
    let rotting = mined(0.2);
    assert!(pristine > 0.0, "a working bay banks the mission's work");
    assert!(
        rotting < pristine,
        "a rotting bay mines slower than a pristine one ({rotting} vs {pristine})"
    );
}

#[test]
fn a_route_toll_wears_the_ship_every_year_of_its_voyage() {
    // Content-depth charters round 13: a charter whose nature wears at a ship
    // exacts a steady per-year drain — hazard's deterministic companion. The
    // coronal tap's radiation-and-heat toll drops morale and hull each year;
    // an ordinary survey exacts nothing.
    use crate::simulation::tick::advance_year;
    let mut data = GameData::load().unwrap();
    // Isolate the toll: no reactive rolls, no threshold beats. Voyage drift
    // still wears both ships, but it wears them identically, so the *difference*
    // is the route's own standing toll.
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.campaign_skeleton.drift_beats.clear();
    data.config.campaign_skeleton.adaptation_beats.clear();
    data.config.campaign_skeleton.crisis_beats.clear();
    data.config.campaign_skeleton.flourish_beats.clear();
    data.config.campaign_skeleton.objective_beats.clear();
    data.config.campaign_skeleton.depopulation_beats.clear();
    let toll = &data.contracts.get("coronal_tap").unwrap().annual_toll;
    assert!(!toll.is_none(), "the coronal tap is a punishing route");
    assert!(
        data.contracts
            .get("deep_vein_survey")
            .unwrap()
            .annual_toll
            .is_none(),
        "an ordinary survey exacts no standing toll"
    );

    let fly = |charter: &str| -> (f32, f32) {
        let picks = crate::state::sim::founding_faction_ids(&data);
        let mut sim = SimState::new_campaign(&data, "preservers", 61, &picks);
        sim.resources.food = 1_000_000; // isolate the toll from famine
        let template = data.contracts.get(charter).unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        sim.contract.as_mut().unwrap().beats.clear();
        let (m0, h0) = (sim.population.morale, sim.ship.hull_integrity);
        for _ in 0..10 {
            advance_year(&mut sim, &data);
        }
        (m0 - sim.population.morale, h0 - sim.ship.hull_integrity)
    };
    let (tapped_morale, tapped_hull) = fly("coronal_tap");
    let (survey_morale, survey_hull) = fly("deep_vein_survey");
    assert!(
        tapped_morale > survey_morale && tapped_hull > survey_hull,
        "the star's reach wears morale and hull faster than a quiet survey \
             (tap {tapped_morale}/{tapped_hull} vs survey {survey_morale}/{survey_hull})"
    );
}

#[test]
fn working_a_mission_sharpens_the_craft_it_leans_on() {
    // Content-depth charters round 33: the reverse of the round-14 objective_condition coupling.
    // A mission's on-station work builds the objective subsystem's knowledge — deep_vein_survey
    // leans on the engineering bay, so months of it master that craft; work before the ship is
    // on-station (Travel) trains nothing.
    let (data, mut sim) = armed(9, "deep_vein_survey");
    assert!(
        data.config
            .subsystems
            .objective_subsystem_training_per_month
            > 0.0,
        "this test needs mission-training enabled"
    );
    assert_eq!(
        sim.contract.as_ref().unwrap().objective_subsystem,
        "engineering_bay"
    );
    // Start the bay's craft below full so the lift is visible.
    sim.subsystems.get_mut("engineering_bay").unwrap().knowledge = 0.5;
    let pre_op = sim.subsystems["engineering_bay"].knowledge;

    // Advance to on-station; before Operation the work trains nothing.
    loop {
        let p = advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
        if p.phase_changed == Some(ContractPhase::Operation) {
            break;
        }
        assert_eq!(
            sim.subsystems["engineering_bay"].knowledge, pre_op,
            "no craft is built before the ship is on-station"
        );
    }
    // The first on-station month sharpens the bay.
    let after_first = sim.subsystems["engineering_bay"].knowledge;
    assert!(
        after_first > pre_op,
        "an operation month sharpens the objective subsystem ({after_first} vs {pre_op})"
    );
    // And more work builds more craft.
    advance_contract(&mut sim, &data.config, 0, 0, 0, 0);
    assert!(
        sim.subsystems["engineering_bay"].knowledge > after_first,
        "another operation month builds the craft further"
    );
}

#[test]
fn a_mission_done_well_teaches_more_than_one_barely_scraped() {
    // Content-depth charters round 25: the lasting *lessons* of a mission scale with
    // how well it went. deep_vein_survey's completion boon lifts engineering
    // knowledge; a clean completion (score 1.0) teaches exactly twice what a
    // barely-scraped one (0.5) does, and both leave the reward's log line.
    let (data, mut clean) = armed(3, "deep_vein_survey");
    let (_d, mut scrappy) = armed(3, "deep_vein_survey");
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    // Hold the bay mid-range so neither gain clamps.
    clean
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .knowledge = 0.5;
    scrappy
        .subsystems
        .get_mut("engineering_bay")
        .unwrap()
        .knowledge = 0.5;

    apply_completion_reward(&mut clean, &template, 1.0);
    apply_completion_reward(&mut scrappy, &template, 0.5);
    let clean_gain = clean.subsystems.get("engineering_bay").unwrap().knowledge - 0.5;
    let scrappy_gain = scrappy.subsystems.get("engineering_bay").unwrap().knowledge - 0.5;
    assert!(clean_gain > 0.0, "a completed survey teaches the crew");
    assert!(
        (clean_gain - 2.0 * scrappy_gain).abs() < 1e-6,
        "half the performance, half the lesson: {clean_gain} vs {scrappy_gain}"
    );
}
