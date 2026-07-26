//! Membership arithmetic: founding shares, rebalancing, growth drift,
//! losing a people, merging one away, and assimilating the drifted.

use super::*;

#[test]
fn founding_splits_population_and_is_deterministic() {
    let (data, sim, picks) = armed(7);
    let sum: u32 = sim.factions.iter().map(|f| f.members).sum();
    assert_eq!(sum, sim.population.count, "members sum to the head count");
    assert_eq!(sim.factions.len(), picks.len());
    assert!(sim.factions.iter().all(|f| f.is_aboard()));

    let again = SimState::new_campaign(&data, "preservers", 7, &picks);
    let a: Vec<_> = sim.factions.iter().map(|f| f.members).collect();
    let b: Vec<_> = again.factions.iter().map(|f| f.members).collect();
    assert_eq!(a, b, "deterministic per (seed, factions)");
}

#[test]
fn rebalance_preserves_shares_and_the_sum_invariant() {
    let (_data, mut sim, _picks) = armed(1);
    let total: u32 = sim.factions.iter().map(|f| f.members).sum();
    let before: Vec<f32> = sim
        .factions
        .iter()
        .map(|f| f.members as f32 / total as f32)
        .collect();

    sim.population.count /= 2;
    sim.rebalance_factions();

    let aboard_sum: u32 = sim
        .factions
        .iter()
        .filter(|f| f.is_aboard())
        .map(|f| f.members)
        .sum();
    assert_eq!(aboard_sum, sim.population.count, "sum invariant holds");
    let now: u32 = sim.factions.iter().map(|f| f.members).sum();
    for (i, f) in sim.factions.iter().enumerate() {
        let share = f.members as f32 / now as f32;
        assert!(
            (share - before[i]).abs() < 0.02,
            "share preserved for {}",
            f.faction_id
        );
    }
}

#[test]
fn demographic_drift_shifts_the_balance_of_power_over_generations() {
    // Content-depth factions round 11: which people runs the ship is not fixed
    // at launch. A fecund people (the Hearth) grows its share over the
    // generations while a people that does not reproduce naturally (the
    // augmented Ascension) dwindles — so a launch minority can become the
    // majority and the dominant faction can flip mid-voyage.
    let data = GameData::load().unwrap();
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        5,
        &crate::state::sim::founding_faction_ids(&data),
    );
    // Start the two peoples level, Ascension a shade ahead.
    sim.factions = vec![fs("ascension_circle", 520), fs("hearth_union", 480)];
    sim.population.count = 1000;
    let share = |sim: &SimState, id: &str| {
        let total: u32 = sim
            .factions
            .iter()
            .filter(|f| f.is_aboard())
            .map(|f| f.members)
            .sum();
        sim.factions
            .iter()
            .find(|f| f.faction_id == id)
            .map_or(0.0, |f| f.members as f32 / total as f32)
    };
    assert_eq!(
        sim.dominant_faction_id(),
        Some("ascension_circle"),
        "the augmented lead at launch"
    );
    let asc0 = share(&sim, "ascension_circle");

    // Twelve generations of demographic drift (rebalancing to the head count).
    for _ in 0..12 {
        sim.apply_faction_demographic_drift(&data);
        sim.rebalance_factions();
    }
    assert!(
        share(&sim, "ascension_circle") < asc0,
        "the augmented dwindle over the centuries"
    );
    assert_eq!(
        sim.dominant_faction_id(),
        Some("hearth_union"),
        "a fecund launch-minority has become the majority"
    );
}

#[test]
fn how_a_people_is_treated_bends_how_it_grows() {
    // Content-depth factions round 13: approval bends demographic growth — the
    // link between the approval meter (r8) and demographic drift (r11). Two
    // peoples of identical nature (same base bias), one cherished and one
    // resented, diverge over the generations: the beloved waxes, the resented
    // wanes, even though nothing about their kind differs.
    let data = GameData::load().unwrap();
    assert!(
        data.config.factions.approval_growth_factor > 0.0,
        "this test needs the approval→growth coupling enabled"
    );
    // Both tend the same base bias; only their standing with the ship differs.
    // (Use one faction id so the base growth_bias is identical for both runs.)
    let grow = |approval: f32| -> u32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            7,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.factions = vec![FactionState {
            faction_id: "meridian_accord".to_string(),
            members: 500,
            status: FactionStatus::Aboard,
            approval,
            mood_band: 0,
        }];
        for _ in 0..12 {
            sim.apply_faction_demographic_drift(&data);
        }
        sim.factions[0].members
    };
    let cherished = grow(0.95);
    let resented = grow(0.05);
    assert!(
        cherished > resented,
        "a cherished people waxes where a resented one wanes \
             (cherished {cherished} vs resented {resented})"
    );
    // Neutral standing leaves growth to nature alone (the base bias only).
    let neutral = grow(0.5);
    assert!(
        cherished > neutral && neutral > resented,
        "approval pushes growth both ways around a neutral baseline"
    );
}

#[test]
fn a_near_total_collapse_wipes_the_smallest_faction() {
    let (_data, mut sim, picks) = armed(1);
    sim.factions = vec![fs(&picks[0], 1), fs(&picks[1], 500), fs(&picks[2], 500)];
    sim.population.count = 2;

    let wiped = sim.rebalance_factions();
    assert_eq!(wiped, vec![picks[0].clone()]);
    assert_eq!(sim.factions[0].status, FactionStatus::WipedOut);
    let aboard_sum: u32 = sim
        .factions
        .iter()
        .filter(|f| f.is_aboard())
        .map(|f| f.members)
        .sum();
    assert_eq!(aboard_sum, 2);
}

#[test]
fn a_tiny_drifted_faction_is_assimilated_only_when_drift_is_high() {
    let (data, mut sim, picks) = armed(1);
    let seed_factions = || vec![fs(&picks[0], 40), fs(&picks[1], 480), fs(&picks[2], 480)];

    // Low drift: the small faction holds on.
    sim.factions = seed_factions();
    sim.population.count = 1000;
    sim.population.cultural_drift = 0.3;
    sim.assimilate_drifted_factions(&data);
    assert!(
        sim.factions.iter().all(|f| f.is_aboard()),
        "drift 0.3 spares it"
    );

    // High drift: the 4% faction (< 5% threshold) folds into a larger one.
    sim.factions = seed_factions();
    sim.population.cultural_drift = 0.8;
    sim.assimilate_drifted_factions(&data);
    assert_eq!(sim.factions[0].status, FactionStatus::Assimilated);
    assert_eq!(sim.factions[0].members, 0);
    let aboard_sum: u32 = sim
        .factions
        .iter()
        .filter(|f| f.is_aboard())
        .map(|f| f.members)
        .sum();
    assert_eq!(
        aboard_sum, 1000,
        "assimilation transfers, never loses, members"
    );
}

#[test]
fn faction_loss_removes_the_smallest_but_spares_the_last() {
    let (data, mut sim, picks) = armed(1);
    sim.factions = vec![fs(&picks[0], 100), fs(&picks[1], 500), fs(&picks[2], 400)];
    sim.population.count = 1000;

    sim.apply_faction_loss(&data, FactionLossKind::Settled);
    assert_eq!(sim.factions[0].status, FactionStatus::Settled);
    assert_eq!(sim.factions[0].members, 0);
    assert_eq!(
        sim.population.count, 900,
        "the settlers leave the head count"
    );

    // Reduce to a single Aboard faction; it can never be lost this way.
    let mut solo = SimState::new_campaign(&data, "preservers", 2, &picks);
    solo.factions = vec![fs(&picks[0], 1000)];
    solo.population.count = 1000;
    solo.apply_faction_loss(&data, FactionLossKind::Departed);
    assert!(
        solo.factions[0].is_aboard(),
        "the last people are never lost"
    );
    assert_eq!(solo.population.count, 1000);
}

#[test]
fn targeted_faction_loss_sheds_the_named_group_not_the_smallest() {
    let (data, mut sim, picks) = armed(1);
    // Named faction (picks[1]) is the LARGEST, so a smallest-loss would spare
    // it — targeting must remove it anyway (content-depth round 3 schism).
    sim.factions = vec![fs(&picks[0], 100), fs(&picks[1], 500), fs(&picks[2], 400)];
    sim.population.count = 1000;

    sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, &picks[1]);
    assert_eq!(sim.factions[1].status, FactionStatus::Departed);
    assert_eq!(sim.factions[1].members, 0);
    assert_eq!(
        sim.population.count, 500,
        "the departed faction leaves the head count"
    );
    assert!(sim.factions[0].is_aboard() && sim.factions[2].is_aboard());

    // Never the last aboard people, even when named.
    let mut solo = SimState::new_campaign(&data, "preservers", 2, &picks);
    solo.factions = vec![fs(&picks[0], 1000)];
    solo.population.count = 1000;
    solo.apply_faction_loss_by_id(&data, FactionLossKind::Departed, &picks[0]);
    assert!(
        solo.factions[0].is_aboard(),
        "the last people are never lost"
    );
}

#[test]
fn a_people_merging_into_the_majority_consolidates_the_polity() {
    // Content-depth factions round 26: the positive mirror of the departure scar. A
    // tiny drifted remnant folding into the largest people lifts unity (one fewer
    // faultline), scaled by how much of the ship just merged.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let cfg = data.config.factions;
    assert!(
        cfg.assimilation_unity_lift > 0.0,
        "this test needs the assimilation-consolidation coupling enabled"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        9,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.population.unity = 0.5;
    // High enough drift to assimilate, and a remnant below the share threshold.
    sim.population.cultural_drift = cfg.assimilation_drift_threshold + 0.05;
    sim.factions = vec![
        FactionState {
            faction_id: "steel_covenant".to_string(),
            members: 970,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        },
        FactionState {
            faction_id: "hearth_union".to_string(),
            members: 30,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        },
    ];

    let before = sim.population.unity;
    sim.assimilate_drifted_factions(&data);
    assert!(
        sim.factions
            .iter()
            .any(|f| f.faction_id == "hearth_union" && f.status == FactionStatus::Assimilated),
        "the tiny remnant folds into the majority"
    );
    assert!(
        sim.population.unity > before,
        "the merge consolidates the polity ({} -> {})",
        before,
        sim.population.unity
    );
}

#[test]
fn losing_a_whole_people_scars_the_ships_cohesion() {
    // Content-depth factions round 24: a departure wounds cohesion beyond the bodies
    // and the craft — morale and unity both take a hit scaled by the departing
    // people's share of the ship.
    use crate::state::sim::factions::{FactionState, FactionStatus};
    let data = GameData::load().unwrap();
    let scar = data.config.factions.departure_cohesion_scar;
    assert!(
        scar > 0.0,
        "this test needs the departure-scar coupling enabled"
    );

    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        9,
        &crate::state::sim::founding_faction_ids(&data),
    );
    sim.population.morale = 0.6;
    sim.population.unity = 0.6;
    sim.population.count = 1000;
    // Two evenly large peoples: one holds half the ship.
    sim.factions = vec![
        FactionState {
            faction_id: "steel_covenant".to_string(),
            members: 500,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        },
        FactionState {
            faction_id: "hearth_union".to_string(),
            members: 500,
            status: FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        },
    ];

    let (m0, u0) = (sim.population.morale, sim.population.unity);
    sim.apply_faction_loss_by_id(&data, FactionLossKind::Departed, "steel_covenant");
    assert!(
        sim.population.morale < m0,
        "losing a great people wounds morale"
    );
    assert!(sim.population.unity < u0, "…and unity");
    // Half the ship departing scars by scar × 0.5.
    let expected = scar * 0.5;
    assert!(
        (m0 - sim.population.morale - expected).abs() < 1e-4,
        "the scar scales by the departing share ({} vs {expected})",
        m0 - sim.population.morale
    );
}
