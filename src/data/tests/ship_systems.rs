//! The six ship subsystems and the bounded couplings they exert on the sim.

use super::*;

/// Well-formed tiers and buffered families, and every penalty or relief
/// a subsystem applies is a fraction that cannot invert the mechanic.
#[test]
fn the_subsystems_are_authored_and_their_couplings_bounded() {
    let data = GameData::load().unwrap();
    // W5: six subsystems load; each non-empty buffered family is one of the
    // canonical W6 family strings; tiers are well-formed (3, positive cost).
    let canonical_families: std::collections::HashSet<&str> = [
        "exploration_first_contact",
        "diplomacy",
        "engineering",
        "biology_medical",
        "science_anomaly",
        "survival",
        "mystery",
        "comedy",
        "ethics",
        "legacy_drift",
    ]
    .into_iter()
    .collect();
    assert_eq!(data.subsystems.len(), 6, "six ship subsystems");
    for (id, sub) in data.subsystems.iter() {
        if !sub.buffers_family.is_empty() {
            assert!(
                canonical_families.contains(sub.buffers_family.as_str()),
                "subsystem '{id}' buffers a non-canonical family '{}'",
                sub.buffers_family
            );
        }
        // Three bought upgrade tiers, optionally topped by mission-reward
        // versions (2c): the purchasable ladder is exactly three, and any
        // extra tiers above it must be mission rewards.
        assert!(
            sub.tiers.len() >= 3,
            "subsystem '{id}' needs three bought upgrade tiers"
        );
        // Named-version pass: the baseline and every fitting carry a name, and
        // fitting ids are unique within the subsystem (a mission's
        // `grant_fitting` addresses them by id).
        assert!(
            !sub.baseline_name.trim().is_empty(),
            "subsystem '{id}' has no baseline_name"
        );
        let mut fitting_ids = std::collections::HashSet::new();
        for (ti, tier) in sub.tiers.iter().enumerate() {
            let mission_reward = tier.acquisition.is_mission_only();
            // The first three rungs are bought (positive cost); mission-reward
            // versions sit above them and are free (the mission is the price).
            if mission_reward {
                assert!(
                    ti >= 3,
                    "subsystem '{id}' mission-reward version '{}' must sit above the three bought tiers",
                    tier.id
                );
                assert!(
                    tier.cost == crate::data::ResourceDelta::default(),
                    "subsystem '{id}' mission-reward version '{}' carries a price but is never bought",
                    tier.id
                );
            } else {
                assert!(
                    ti < 3,
                    "subsystem '{id}' has a bought tier above the three-rung ladder",
                );
                assert!(
                    tier.cost.credits > 0,
                    "subsystem '{id}' tier cost must be positive"
                );
            }
            // Content-depth subsystems round 5: every tier carries its own
            // upgrade prose, so a rebuild never falls back to the generic
            // shared line.
            assert!(
                !tier.flavor.trim().is_empty(),
                "subsystem '{id}' has a tier with no upgrade flavor"
            );
            assert!(
                !tier.name.trim().is_empty(),
                "subsystem '{id}' has a fitting with no name"
            );
            assert!(
                fitting_ids.insert(tier.id.as_str()),
                "subsystem '{id}' has a duplicate fitting id '{}'",
                tier.id
            );
        }
        // Content-depth subsystem coverage: every subsystem has at least one
        // knowledge-crisis event, so a module's know-how decaying always has
        // a beat to surface.
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.knowledge_below.iter().any(|g| &g.id == id)),
            "subsystem '{id}' has no knowledge_below crisis event"
        );
        // Content-depth subsystem coverage (round 4): and at least one
        // condition-breakdown event, so a module physically rotting always
        // has a beat to surface — the parallel to the knowledge crisis above.
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.condition_below.iter().any(|g| &g.id == id)),
            "subsystem '{id}' has no condition_below breakdown event"
        );
    }
    // Content-depth subsystems round 21: the security crisis-mitigation is a gentle
    // positive dampener (a corps quiets danger, never conjures it), and its floor
    // keeps the crisis category from being dampened out of existence.
    let subs_cfg = &data.config.subsystems;
    assert!(
        (0.0..=0.3).contains(&subs_cfg.security_crisis_mitigation),
        "security_crisis_mitigation {} out of the gentle range [0, 0.3]",
        subs_cfg.security_crisis_mitigation
    );
    // Content-depth subsystems round 22: the cultural morale swing is gentle, like
    // the habitat one it mirrors — a pillar of spirits, not a lever that swamps them.
    assert!(
        (0.0..=0.05).contains(&subs_cfg.education_morale_swing),
        "education_morale_swing {} out of the gentle range [0, 0.05]",
        subs_cfg.education_morale_swing
    );
    // Content-depth subsystems round 23: the medical renewal penalty is a fraction
    // (a failing bay loses *some* of the young, never all — a collapsed infirmary
    // must not zero out births).
    assert!(
        (0.0..=0.8).contains(&subs_cfg.medical_renewal_penalty),
        "medical_renewal_penalty {} out of range [0, 0.8]",
        subs_cfg.medical_renewal_penalty
    );
    // Content-depth subsystems round 24: the engineering→hull decay penalty is a
    // bounded acceleration (a wrecked bay wears the hull faster, but not so much it
    // shatters a sound ship in a season).
    assert!(
        (0.0..=1.5).contains(&subs_cfg.engineering_hull_decay_penalty),
        "engineering_hull_decay_penalty {} out of range [0, 1.5]",
        subs_cfg.engineering_hull_decay_penalty
    );
    // Content-depth subsystems round 26: the engineering→fabrication penalty is a
    // fraction in [0, 1] (a wrecked bay fabricates less, but never a negative yield;
    // 1 means a fully wrecked bay fabricates nothing beyond the one-part floor).
    assert!(
        (0.0..=1.0).contains(&subs_cfg.engineering_fabrication_penalty),
        "engineering_fabrication_penalty {} out of range [0, 1]",
        subs_cfg.engineering_fabrication_penalty
    );
    // Content-depth subsystems round 34: the field-repair penalty is a fraction in [0, 1] — a
    // fully wrecked bay makes the weakest field mend (at 1, only the residual gain), but even it
    // patches something, and a sound bay always makes a full one.
    assert!(
        (0.0..=1.0).contains(&subs_cfg.engineering_field_repair_penalty),
        "engineering_field_repair_penalty {} out of range [0, 1]",
        subs_cfg.engineering_field_repair_penalty
    );
    // Content-depth subsystems round 30: the engineering→fuel-regen penalty is a fraction in
    // [0, 1] (a wrecked bay scoops less, but never a negative — fuel regen floors at zero).
    assert!(
        (0.0..=1.0).contains(&subs_cfg.engineering_fuel_regen_penalty),
        "engineering_fuel_regen_penalty {} out of range [0, 1]",
        subs_cfg.engineering_fuel_regen_penalty
    );
    // Content-depth subsystems round 27: the education→training penalty must sit strictly
    // below 1, so even a wrecked academy still teaches something and a crew can bootstrap
    // its schools back — a training deadlock (0 gain forever) would be unrecoverable.
    assert!(
        (0.0..1.0).contains(&subs_cfg.education_training_penalty),
        "education_training_penalty {} must be in [0, 1) so a wrecked academy still teaches",
        subs_cfg.education_training_penalty
    );
    // Content-depth subsystems round 28: the security→ideology-spread relief must sit
    // strictly below 1, so even a perfect corps only softens the strain of a divided polity
    // and never wholly cancels the governance drain a genuine split inflicts.
    assert!(
        (0.0..1.0).contains(&subs_cfg.ideology_spread_security_relief),
        "ideology_spread_security_relief {} must be in [0, 1) so a corps never fully cancels the drain",
        subs_cfg.ideology_spread_security_relief
    );
    // Content-depth subsystems round 32: the rival-friction relief is a fraction in [0, 1) —
    // even a perfect corps only softens a standing rivalry, never abolishes it.
    assert!(
        (0.0..1.0).contains(&subs_cfg.security_rival_friction_relief),
        "security_rival_friction_relief {} must be in [0, 1) so a corps never fully cancels the grind",
        subs_cfg.security_rival_friction_relief
    );
    // Content-depth subsystems round 25: the medical adaptation resistance is a
    // fraction below 1 (even a perfect infirmary only *slows* the shipborn drift, it
    // never wholly stops the bodies adapting to the ship).
    assert!(
        (0.0..1.0).contains(&data.config.voyage_drift.medical_adaptation_resistance),
        "medical_adaptation_resistance {} must be a fraction in [0, 1)",
        data.config.voyage_drift.medical_adaptation_resistance
    );
    // Content-depth subsystems round 29: the agriculture adaptation resistance, the
    // biosphere twin, is likewise a fraction below 1 (a living farm only *slows* the drift).
    assert!(
        (0.0..1.0).contains(&data.config.voyage_drift.agriculture_adaptation_resistance),
        "agriculture_adaptation_resistance {} must be a fraction in [0, 1)",
        data.config.voyage_drift.agriculture_adaptation_resistance
    );
    // Content-depth subsystems round 31: the medical life-support relief is a fraction below 1
    // (even a perfect infirmary only *saves some* of the asphyxiating; it cannot make air).
    assert!(
        (0.0..1.0).contains(&subs_cfg.medical_life_support_relief),
        "medical_life_support_relief {} must be a fraction in [0, 1)",
        subs_cfg.medical_life_support_relief
    );
    // Content-depth charters round 33: mission-training is a *small* per-month knowledge gain in
    // [0, 0.1) — a mission runs many months, so even a modest rate masters a craft over a long
    // voyage without a single month vaulting the subsystem to expert.
    assert!(
        (0.0..0.1).contains(&subs_cfg.objective_subsystem_training_per_month),
        "objective_subsystem_training_per_month {} must be a small monthly gain [0, 0.1)",
        subs_cfg.objective_subsystem_training_per_month
    );
    // Content-depth subsystems round 33: the knowledge-upkeep reduction is a fraction in [0, 1)
    // — a mastered module decays slower, but even perfect craft only slows the rot, never stops
    // it (a full 1.0 would make a well-known module immortal).
    assert!(
        (0.0..1.0).contains(&subs_cfg.knowledge_decay_reduction),
        "knowledge_decay_reduction {} must be a fraction in [0, 1)",
        subs_cfg.knowledge_decay_reduction
    );
    if subs_cfg.security_crisis_mitigation > 0.0 {
        assert!(
            subs_cfg.crisis_weight_floor > 0.0,
            "a crisis-weight floor must be set so security can never silence danger"
        );
    }
}
