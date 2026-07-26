//! The line renews: characters age and die across a voyage no one
//! survives, and the ship remarks on a long reign or a dwindled house.

use super::*;

#[test]
fn characters_age_die_and_the_line_renews_over_a_long_voyage() {
    // Real-time loop follow-up: aging is yearly, death is a monthly age-scaled
    // roll, and yearly births keep the line viable. Over a long crossing the
    // founders pass on, leadership changes hands, new members come of age, and
    // the dynasty survives — always led while anyone lives.
    let (data, mut sim) = provisioned(3, 1.0);
    let founder_id = sim.dynasty.leader().unwrap().id;
    let founding_next_id = sim.dynasty.next_member_id;
    let founding_ages: Vec<u32> = sim.dynasty.members.iter().map(|m| m.age).collect();

    for _ in 0..(120 * 12) {
        sim.pending_event = None;
        sim.pending_dilemma = None;
        advance_months(&mut sim, &data, 1);
        if sim.dynasty.extinct {
            break;
        }
    }

    assert!(
        !sim.dynasty.extinct,
        "a renewing line survives the crossing"
    );
    assert!(
        sim.dynasty.leader().is_some(),
        "a living dynasty is always led"
    );
    assert_ne!(
        sim.dynasty.leader().unwrap().id,
        founder_id,
        "the founding leader did not reign for 120 years"
    );
    assert!(
        sim.dynasty.next_member_id > founding_next_id,
        "new members came of age to renew the line"
    );
    // The surviving members are not the founding cohort frozen in time.
    let current_ages: Vec<u32> = sim.dynasty.members.iter().map(|m| m.age).collect();
    assert_ne!(
        current_ages, founding_ages,
        "the roster aged and turned over"
    );
}

#[test]
fn an_enduring_reign_earns_a_long_reign_beat_once() {
    // Content-depth campaign skeleton round 19: a leader who beats the odds of
    // continuous mortality and holds the chair for `long_reign_years` earns a beat,
    // once — the hopeful mirror of the succession beat.
    let (data, mut sim) = provisioned(3, 1.0);
    // Keep the leader young so no death/retirement resets the reign mid-test.
    for member in &mut sim.dynasty.members {
        if member.is_leader {
            member.age = 40;
        }
    }
    let threshold = data.config.campaign_skeleton.long_reign_years;
    assert!(threshold > 0, "the long-reign beat must be configured");
    sim.dynasty.leader_reign_years = threshold;
    assert!(
        !sim.dynasty.long_reign_marked,
        "the reign is not yet marked"
    );

    sim.pending_event = None;
    sim.pending_dilemma = None;
    advance_months(&mut sim, &data, 1);
    assert!(
        sim.dynasty.long_reign_marked,
        "an enduring reign is marked with a beat"
    );

    // A fresh succession re-arms it for the next reign.
    crate::simulation::succession::install_successor(&mut sim.dynasty, &data.config);
    assert!(
        !sim.dynasty.long_reign_marked && sim.dynasty.leader_reign_years == 0,
        "a handoff starts a new, unmarked reign"
    );
}

#[test]
fn a_dwindled_line_forces_a_dynasty_crisis_beat_once() {
    // Content-depth campaign skeleton round 20: when the founding line dwindles to
    // the crisis size, a beat marks the ship's brush with the end of its dynasty.
    let (data, mut sim) = provisioned(3, 1.0);
    // Thin the line into crisis (the leader stays, so no succession churn).
    sim.dynasty.members.truncate(2);
    assert!(
        (sim.dynasty.members.len() as u32) <= data.config.campaign_skeleton.dynasty_crisis_size
    );
    assert!(!sim.dynasty.dynasty_crisis_marked, "not yet marked");

    sim.pending_event = None;
    sim.pending_dilemma = None;
    advance_months(&mut sim, &data, 1);
    assert!(
        sim.dynasty.dynasty_crisis_marked,
        "the near-end of the founding line is marked with a beat"
    );
}
