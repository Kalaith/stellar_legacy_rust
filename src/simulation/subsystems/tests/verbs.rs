//! Repair, upgrade, install and train: what each verb demands of the crew
//! and what it leaves behind, plus the craft handed down a generation.

use super::*;

#[test]
fn repair_needs_living_expertise() {
    let (data, mut sim) = campaign(2);
    sim.resources.minerals = 100_000;
    sim.ship.spare_parts = 100;
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.3;

    // Below the knowledge threshold: refused, and nothing is spent.
    sim.subsystems.get_mut("medical_bay").unwrap().knowledge = 0.1;
    let minerals_before = sim.resources.minerals;
    assert!(repair_subsystem(&mut sim, &data, "medical_bay").is_err());
    assert_eq!(
        sim.resources.minerals, minerals_before,
        "a refused repair charges nothing"
    );

    // Above it: the repair lands and spends consumables.
    sim.subsystems.get_mut("medical_bay").unwrap().knowledge = 0.9;
    repair_subsystem(&mut sim, &data, "medical_bay").unwrap();
    assert!(sim.subsystems["medical_bay"].condition > 0.3);
    assert!(sim.resources.minerals < minerals_before);
}

#[test]
fn a_repair_draws_its_line_from_the_pool_not_the_flat_fallback() {
    // Content-depth voice round 9: the field-repair verb fires many times a
    // voyage, so it draws a varied pooled line naming the module, not the one
    // flat "patched back toward working order" string it used to reprint.
    let (data, mut sim) = campaign(4);
    sim.resources.minerals = 100_000;
    sim.ship.spare_parts = 100;
    let bay = data.subsystems.get("medical_bay").unwrap().name.clone();
    sim.subsystems.get_mut("medical_bay").unwrap().knowledge = 0.9;
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.3;

    let log_before = sim.log.len();
    repair_subsystem(&mut sim, &data, "medical_bay").unwrap();
    let line = &sim.log[log_before].text;
    assert!(line.contains(&bay), "the repair line names the module");
    assert!(
        data.config
            .flavor
            .subsystem_repair
            .iter()
            .any(|t| line == &t.replace("{name}", &bay)),
        "the line comes from the pool, not the flat fallback: {line}"
    );
}

#[test]
fn a_sound_engineering_bay_makes_a_better_field_repair() {
    // Content-depth subsystems round 34: the repair companion to the round-7 decay keystone. A
    // field (underway) repair is made with the bay's fabricators, so a sound bay mends the
    // medical ward further than a failing one — but even a failing bay patches something. (In
    // port, full facilities take repairs whole regardless, so this is exercised only underway.)
    assert!(
        GameData::load()
            .unwrap()
            .config
            .subsystems
            .engineering_field_repair_penalty
            > 0.0,
        "this test needs the field-repair coupling enabled"
    );

    let field_restore = |eng_condition: f32| -> f32 {
        let (data, mut sim) = campaign(6);
        // Underway, so field repairs (capped at the field ceiling), not whole dock repairs.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(crate::simulation::contract::start_contract(&template, &sim));
        sim.resources.minerals = 100_000;
        sim.ship.spare_parts = 100;
        sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.3;
        sim.subsystems.get_mut("medical_bay").unwrap().knowledge = 0.9;
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = eng_condition;
        repair_subsystem(&mut sim, &data, "medical_bay").unwrap();
        sim.subsystems["medical_bay"].condition - 0.3
    };

    let sound = field_restore(1.0); // a sound bay makes the full field gain
    let failing = field_restore(0.2); // a failing bay can only patch
    assert!(
        sound > failing,
        "a sound engineering bay mends the ward further ({sound} vs {failing})"
    );
    assert!(
        failing > 0.0,
        "but even a failing bay patches something ({failing})"
    );
}

#[test]
fn a_stronger_medical_bay_softens_biology_damage() {
    let (data, mut sim) = campaign(3);

    // Baseline tier 0 buffers nothing.
    let (r0, _, _) = buffered_deltas(
        &sim,
        &data,
        "biology_medical",
        ResourceDelta {
            food: -100,
            ..Default::default()
        },
        ShipDelta::default(),
        PopulationDelta::default(),
    );
    assert_eq!(r0.food, -100, "tier 0 leaves the harm in full");

    // Tier 2 at full condition scales negatives by 1 - severity_reduction;
    // positive components pass untouched.
    {
        let s = sim.subsystems.get_mut("medical_bay").unwrap();
        s.tier = 2;
        s.condition = 1.0;
    }
    let sr = data.subsystems.get("medical_bay").unwrap().tiers[1].severity_reduction;
    let factor = 1.0 - sr;
    let (r2, _, p2) = buffered_deltas(
        &sim,
        &data,
        "biology_medical",
        ResourceDelta {
            food: -100,
            ..Default::default()
        },
        ShipDelta::default(),
        PopulationDelta {
            count: -50,
            morale: 0.1,
            ..Default::default()
        },
    );
    assert_eq!(r2.food, (-100.0f32 * factor) as i64, "negative food scaled");
    assert_eq!(
        p2.count,
        (-50.0f32 * factor) as i32,
        "negative count scaled"
    );
    assert_eq!(p2.morale, 0.1, "positive morale untouched");
    assert!(
        r2.food > r0.food,
        "the upgrade measurably reduces the damage"
    );
}

#[test]
fn upgrade_is_port_only_and_caps_at_tier_three() {
    use crate::simulation::contract::start_contract;
    let (data, mut sim) = campaign(4);
    sim.resources.credits = 1_000_000;
    sim.resources.minerals = 1_000_000;

    // Underway: refused.
    let template = data.contracts.get("deep_vein_survey").unwrap().clone();
    sim.contract = Some(start_contract(&template, &sim));
    assert!(upgrade_subsystem(&mut sim, &data, "medical_bay").is_err());

    // In port: climbs to tier 3 then caps.
    sim.contract = None;
    for _ in 0..3 {
        upgrade_subsystem(&mut sim, &data, "medical_bay").unwrap();
    }
    assert_eq!(sim.subsystems["medical_bay"].tier, 3);
    assert!(
        upgrade_subsystem(&mut sim, &data, "medical_bay").is_err(),
        "tier caps at 3"
    );
}

#[test]
fn an_upgrade_logs_its_tier_specific_flavor() {
    // Content-depth subsystems round 5: each rebuild reads in the module's
    // own voice, not the shared "rebuilt stronger" line — and the tiers read
    // differently from one another (a real escalation, not a repeat).
    let (data, mut sim) = campaign(9);
    sim.resources.credits = 1_000_000;
    sim.resources.minerals = 1_000_000;

    let t1_flavor = data.subsystems.get("engineering_bay").unwrap().tiers[0]
        .flavor
        .clone();
    assert!(!t1_flavor.is_empty());

    upgrade_subsystem(&mut sim, &data, "engineering_bay").unwrap();
    assert!(
        sim.log.iter().any(|l| l.text == t1_flavor),
        "the tier-1 rebuild logs its own flavor line"
    );
    assert!(
        !sim.log.iter().any(|l| l.text.contains("rebuilt stronger")),
        "an authored tier never falls back to the generic line"
    );

    // Tier 2 reads differently from tier 1 (escalation, no repetition tell).
    let t2_flavor = data.subsystems.get("engineering_bay").unwrap().tiers[1]
        .flavor
        .clone();
    assert_ne!(t1_flavor, t2_flavor);
}

#[test]
fn a_mission_reward_version_installs_only_once_unlocked() {
    let (data, mut sim) = campaign(11);
    sim.resources.credits = 1_000_000;
    sim.resources.minerals = 1_000_000;
    // Climb engineering to the top bought tier (3).
    for _ in 0..3 {
        upgrade_subsystem(&mut sim, &data, "engineering_bay").unwrap();
    }
    assert_eq!(sim.subsystems["engineering_bay"].tier, 3);
    // The mission-reward 4th version can't be bought…
    assert!(upgrade_subsystem(&mut sim, &data, "engineering_bay").is_err());
    // …nor installed before a mission unlocks it.
    assert!(install_fitting(&mut sim, &data, "engineering_bay").is_err());
    // Grant it, then it fits — free, and the unlock is spent.
    sim.ship
        .unlocked_fittings
        .push("nanolathe_forge".to_owned());
    let credits_before = sim.resources.credits;
    install_fitting(&mut sim, &data, "engineering_bay").unwrap();
    assert_eq!(sim.subsystems["engineering_bay"].tier, 4);
    assert_eq!(
        sim.resources.credits, credits_before,
        "a recovered version is free"
    );
    assert!(
        !sim.ship
            .unlocked_fittings
            .iter()
            .any(|f| f == "nanolathe_forge"),
        "the unlock is consumed on install"
    );
}

#[test]
fn an_untrained_line_loses_then_relearns_the_repair() {
    let (data, mut sim) = campaign(5);
    sim.resources.minerals = 100_000;
    sim.ship.spare_parts = 100;
    sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.3;
    let required = data
        .subsystems
        .get("medical_bay")
        .unwrap()
        .repair_knowledge_required;

    // Education tier 0, no training: knowledge falls below the threshold and
    // the subsystem becomes unrepairable.
    for _ in 0..3 {
        transmit_knowledge(&mut sim, &data);
    }
    assert!(sim.subsystems["medical_bay"].knowledge < required);
    assert!(
        repair_subsystem(&mut sim, &data, "medical_bay").is_err(),
        "no one remembers how to mend it"
    );

    // Training a new cohort rebuilds the knowledge and the ability.
    sim.resources.credits = 100_000;
    for _ in 0..3 {
        train_subsystem_knowledge(&mut sim, &data, "medical_bay").unwrap();
    }
    assert!(repair_subsystem(&mut sim, &data, "medical_bay").is_ok());
}

#[test]
fn condition_decays_and_knowledge_transmits_with_education() {
    let (data, mut sim) = campaign(1);

    let before = sim.subsystems["medical_bay"].condition;
    decay_subsystems(&mut sim, &data, 1.0);
    assert!(
        sim.subsystems["medical_bay"].condition < before,
        "condition falls with the years"
    );

    // No schooling: a generation loses knowledge.
    let k0 = sim.subsystems["medical_bay"].knowledge;
    transmit_knowledge(&mut sim, &data);
    assert!(
        sim.subsystems["medical_bay"].knowledge < k0,
        "knowledge dies with an untaught generation"
    );

    // Max education tier: transmission outweighs the decay (net positive).
    sim.subsystems.get_mut("education_culture").unwrap().tier = 3;
    let k1 = sim.subsystems["medical_bay"].knowledge;
    transmit_knowledge(&mut sim, &data);
    assert!(
        sim.subsystems["medical_bay"].knowledge > k1,
        "a schooled generation carries knowledge forward"
    );
}

#[test]
fn a_crumbling_archive_passes_less_of_the_founding_craft_forward() {
    // Content-depth subsystems round 13: education is the knowledge keystone —
    // its condition scales how well every module's knowledge transmits to the
    // next generation. At the same schooling tier, a vivid archive carries the
    // craft forward better than a crumbling one, and a pristine archive matches
    // the untouched baseline.
    let data = GameData::load().unwrap();
    assert!(
        data.config
            .subsystems
            .education_transmission_condition_penalty
            > 0.0,
        "this test needs the education-condition coupling enabled"
    );

    let transmit_at = |edu_condition: f32| -> f32 {
        let (_, mut sim) = campaign(4);
        // A high schooling tier so transmission dominates, at a set archive state.
        let edu = sim.subsystems.get_mut("education_culture").unwrap();
        edu.tier = 3;
        edu.condition = edu_condition;
        // A module whose knowledge starts mid-range, so the generational change
        // is visible either way.
        sim.subsystems.get_mut("medical_bay").unwrap().knowledge = 0.5;
        transmit_knowledge(&mut sim, &data);
        sim.subsystems["medical_bay"].knowledge
    };

    let vivid = transmit_at(1.0);
    let crumbling = transmit_at(0.2);
    assert!(
        vivid > crumbling,
        "a vivid archive carries the craft forward better than a crumbling one \
             (vivid {vivid} vs crumbling {crumbling})"
    );
}

#[test]
fn a_devoted_people_keeps_its_domain_sharper_than_a_resentful_one() {
    // Content-depth factions round 12: a module's tending faction modulates its
    // decay by their mood, closing the neglect → sour → rot spiral. The Verdant
    // Kin tend agriculture; a year under a devoted Kin wears the farm less than
    // a year under a resentful one, all else equal.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    assert!(
        data.config.subsystems.tender_approval_decay_scale > 0.0,
        "this test needs the tender-approval coupling enabled"
    );

    let wear_farm = |approval: f32| -> f32 {
        let (_, mut sim) = campaign(8);
        // A single aboard people that tends the farm, at the given mood.
        sim.factions = vec![FactionState {
            faction_id: "verdant_kin".to_string(),
            members: sim.population.count,
            status: FactionStatus::Aboard,
            approval,
            mood_band: 0,
        }];
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.8;
        // Hold the keystone neutral so only the tenders' mood differs.
        sim.subsystems.get_mut("engineering_bay").unwrap().condition = 0.5;
        decay_subsystems(&mut sim, &data, 1.0);
        0.8 - sim.subsystems["agriculture"].condition
    };

    let devoted = wear_farm(0.95);
    let resentful = wear_farm(0.05);
    assert!(
        resentful > devoted,
        "a resentful people lets its farm rot faster than a devoted one \
             (resentful {resentful} vs devoted {devoted})"
    );
}
