//! The campaign skeleton: every beat must be able to draw a real event, or
//! the voyage would reach its appointed moment and have nothing to say.

use super::*;

/// The phase and dead-air pools a beat draws from.
#[test]
fn every_beat_pool_family_has_authored_events() {
    let data = GameData::load().unwrap();
    // Content-depth campaign-skeleton coupling: every family a beat pool can
    // draw must have authored events, or a beat could land on an empty pool.
    let families: std::collections::HashSet<&String> =
        data.events.iter().map(|(_, e)| &e.family).collect();
    let sk = &data.config.campaign_skeleton;
    for fam in sk
        .travel_pool
        .iter()
        .chain(&sk.operation_pool)
        .chain(&sk.return_pool)
        .chain(&sk.any_pool)
        .chain(&sk.early_pool)
        .chain(&sk.mid_pool)
        .chain(&sk.late_pool)
        .chain(&sk.dead_air_pool)
    {
        assert!(
            families.contains(fam),
            "campaign_skeleton pool family '{fam}' has no events"
        );
    }
}

/// The threshold, crisis, recovery and milestone beats, each of which
/// names its own family and its own arming threshold.
#[test]
fn every_scripted_beat_names_a_stocked_family() {
    let data = GameData::load().unwrap();
    let sk = &data.config.campaign_skeleton;
    let families = authored_families(&data);
    // Content-depth threshold beats: each family they draw from must have
    // events, and thresholds must be ascending in (0, 1] so each fires once
    // in order. Same rules for drift (round 2) and adaptation (round 3).
    for (beats, family, label) in [
        (&sk.drift_beats, &sk.drift_beat_family, "drift"),
        (
            &sk.adaptation_beats,
            &sk.adaptation_beat_family,
            "adaptation",
        ),
    ] {
        if beats.is_empty() {
            continue;
        }
        assert!(
            families.contains(family),
            "campaign_skeleton {label}_beat_family '{family}' has no events"
        );
        assert!(
            beats.windows(2).all(|w| w[0] < w[1]),
            "campaign_skeleton {label}_beats must be strictly ascending"
        );
        assert!(
            beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton {label}_beats must be within (0, 1]"
        );
    }
    // Content-depth round 6: crisis beats are the DESCENDING mirror — the
    // ship's cohesion falling past each level in turn — so the same rules
    // hold but the thresholds must be strictly descending.
    if !sk.crisis_beats.is_empty() {
        assert!(
            families.contains(&sk.crisis_beat_family),
            "campaign_skeleton crisis_beat_family '{}' has no events",
            sk.crisis_beat_family
        );
        assert!(
            sk.crisis_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton crisis_beats must be strictly descending"
        );
        assert!(
            sk.crisis_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton crisis_beats must be within (0, 1]"
        );
        // Content-depth round 13: the recovery threshold must sit clear above
        // the highest crisis threshold, so a fractured ship must genuinely climb
        // out (a hysteresis band where neither beat fires) before it mends.
        if !sk.recovery_beat_family.is_empty() {
            let worst_crisis = sk.crisis_beats.iter().cloned().fold(0.0_f32, f32::max);
            assert!(
                sk.recovery_beat_threshold > worst_crisis && sk.recovery_beat_threshold <= 1.0,
                "recovery_beat_threshold {} must sit above the crisis band {worst_crisis}",
                sk.recovery_beat_threshold
            );
        }
    }
    // Content-depth round 13: the recovery beat's family must have events.
    if !sk.recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.recovery_beat_family),
            "campaign_skeleton recovery_beat_family '{}' has no events",
            sk.recovery_beat_family
        );
    }
    // Content-depth campaign-skeleton round 29: despair beats are the descending
    // morale-collapse pole of the flourish beats — a family with events, and strictly
    // descending thresholds in (0, 1], each fired once as spirits sink past it.
    if !sk.despair_beats.is_empty() {
        assert!(
            families.contains(&sk.despair_beat_family),
            "campaign_skeleton despair_beat_family '{}' has no events",
            sk.despair_beat_family
        );
        assert!(
            sk.despair_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton despair_beats must be strictly descending"
        );
        assert!(
            sk.despair_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton despair_beats must be within (0, 1]"
        );
    }
    // Content-depth campaign-skeleton round 30: the heartening-recovery beat, the morale twin
    // of the it13/it28 recovery beats. Its family must have events, and its threshold must sit
    // clear above the worst despair-collapse line (hysteresis) and at most 1.0.
    if !sk.heartening_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.heartening_recovery_beat_family),
            "campaign_skeleton heartening_recovery_beat_family '{}' has no events",
            sk.heartening_recovery_beat_family
        );
        let worst_despair = sk.despair_beats.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            sk.heartening_recovery_beat_threshold > worst_despair
                && sk.heartening_recovery_beat_threshold <= 1.0,
            "heartening_recovery_beat_threshold {} must sit above the despair band {worst_despair}",
            sk.heartening_recovery_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 31: the covenant-recovery beat, the loyalty twin
    // of the it13/it28/it30 recovery beats. Its family must have events, and its threshold must
    // sit clear above the worst loyalty-collapse line (hysteresis) and at most 1.0.
    if !sk.loyalty_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.loyalty_recovery_beat_family),
            "campaign_skeleton loyalty_recovery_beat_family '{}' has no events",
            sk.loyalty_recovery_beat_family
        );
        let worst_loyalty = sk.loyalty_beats.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            sk.loyalty_recovery_beat_threshold > worst_loyalty
                && sk.loyalty_recovery_beat_threshold <= 1.0,
            "loyalty_recovery_beat_threshold {} must sit above the collapse band {worst_loyalty}",
            sk.loyalty_recovery_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 28: the governance-recovery beat, the stability
    // twin of the it13 unity recovery beat. Its family must have events; its threshold must
    // sit clear above the worst stability-collapse line (hysteresis) and at most 1.0; and a
    // `stability_above`-gated event (gated at or below the threshold, so it is eligible the
    // moment the recovery fires) must exist to surface the reckoning on theme.
    if !sk.stability_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.stability_recovery_beat_family),
            "campaign_skeleton stability_recovery_beat_family '{}' has no events",
            sk.stability_recovery_beat_family
        );
        let worst_stability = sk.stability_beats.iter().cloned().fold(0.0_f32, f32::max);
        assert!(
            sk.stability_recovery_beat_threshold > worst_stability
                && sk.stability_recovery_beat_threshold <= 1.0,
            "stability_recovery_beat_threshold {} must sit above the collapse band {worst_stability}",
            sk.stability_recovery_beat_threshold
        );
        assert!(
            data.events.iter().any(|(_, e)| e
                .stability_above
                .is_some_and(|t| t <= sk.stability_recovery_beat_threshold)),
            "the governance-recovery beat needs a stability_above event eligible at its threshold"
        );
    }
    // Content-depth round 14: loyalty-collapse beats are the DESCENDING mirror
    // on legacy_loyalty — strictly descending, in range, family with events.
    if !sk.loyalty_beats.is_empty() {
        assert!(
            families.contains(&sk.loyalty_beat_family),
            "campaign_skeleton loyalty_beat_family '{}' has no events",
            sk.loyalty_beat_family
        );
        assert!(
            sk.loyalty_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton loyalty_beats must be strictly descending"
        );
        assert!(
            sk.loyalty_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton loyalty_beats must be within (0, 1]"
        );
    }
    // Content-depth round 16: the reputation beat's family must have events, its
    // trait must be one some outcome nudges, and its band thresholds must order.
    if !sk.reputation_beat_family.is_empty() {
        assert!(
            families.contains(&sk.reputation_beat_family),
            "campaign_skeleton reputation_beat_family '{}' has no events",
            sk.reputation_beat_family
        );
        assert!(
            data.events.iter().any(|(_, e)| e.outcomes.iter().any(|o| o
                .reputation_deltas
                .iter()
                .any(|r| r.id == sk.reputation_beat_trait))),
            "campaign_skeleton reputation_beat_trait '{}' no outcome nudges",
            sk.reputation_beat_trait
        );
        assert!(
            sk.reputation_beat_low < sk.reputation_beat_high
                && (0.0..=1.0).contains(&sk.reputation_beat_low)
                && (0.0..=1.0).contains(&sk.reputation_beat_high),
            "campaign_skeleton reputation beat bands must order within [0, 1]"
        );
    }
    // Content-depth round 15: stability beats are the DESCENDING governance
    // mirror — strictly descending, in range, family with events.
    if !sk.stability_beats.is_empty() {
        assert!(
            families.contains(&sk.stability_beat_family),
            "campaign_skeleton stability_beat_family '{}' has no events",
            sk.stability_beat_family
        );
        assert!(
            sk.stability_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton stability_beats must be strictly descending"
        );
        assert!(
            sk.stability_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton stability_beats must be within (0, 1]"
        );
    }
    // Content-depth round 8: flourish beats are the ASCENDING positive pole —
    // morale climbing past each level in turn — so the thresholds must be
    // strictly ascending and in range, and the family must have events.
    if !sk.flourish_beats.is_empty() {
        assert!(
            families.contains(&sk.flourish_beat_family),
            "campaign_skeleton flourish_beat_family '{}' has no events",
            sk.flourish_beat_family
        );
        assert!(
            sk.flourish_beats.windows(2).all(|w| w[0] < w[1]),
            "campaign_skeleton flourish_beats must be strictly ascending"
        );
        assert!(
            sk.flourish_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton flourish_beats must be within [0, 1]"
        );
    }
    // Content-depth round 12: depopulation beats — founding-fraction thresholds
    // the crew falls past in turn, so strictly descending and in range, family
    // with events.
    if !sk.depopulation_beats.is_empty() {
        assert!(
            families.contains(&sk.depopulation_beat_family),
            "campaign_skeleton depopulation_beat_family '{}' has no events",
            sk.depopulation_beat_family
        );
        assert!(
            sk.depopulation_beats.windows(2).all(|w| w[0] > w[1]),
            "campaign_skeleton depopulation_beats must be strictly descending"
        );
        assert!(
            sk.depopulation_beats
                .iter()
                .all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton depopulation_beats must be within (0, 1]"
        );
    }
    // Content-depth round 17: subsystem-collapse beats — each names a real
    // module, a red line in (0, 1], and a family with events.
    for beat in &sk.subsystem_beats {
        assert!(
            data.subsystems.get(&beat.subsystem).is_some(),
            "campaign_skeleton subsystem_beat names unknown module '{}'",
            beat.subsystem
        );
        assert!(
            beat.threshold > 0.0 && beat.threshold <= 1.0,
            "campaign_skeleton subsystem_beat '{}' threshold {} must be within (0, 1]",
            beat.subsystem,
            beat.threshold
        );
        assert!(
            families.contains(&beat.family),
            "campaign_skeleton subsystem_beat '{}' family '{}' has no events",
            beat.subsystem,
            beat.family
        );
    }
    // Content-depth round 9: objective-progress beats — mission-fraction
    // milestones, ascending and in range, family with events.
    if !sk.objective_beats.is_empty() {
        assert!(
            families.contains(&sk.objective_beat_family),
            "campaign_skeleton objective_beat_family '{}' has no events",
            sk.objective_beat_family
        );
        assert!(
            sk.objective_beats.windows(2).all(|w| w[0] < w[1]),
            "campaign_skeleton objective_beats must be strictly ascending"
        );
        assert!(
            sk.objective_beats.iter().all(|&t| (0.0..=1.0).contains(&t)),
            "campaign_skeleton objective_beats must be within [0, 1]"
        );
    }
    // Content-depth round 7: the periodic anniversary beat needs a family
    // with events when it is switched on.
    if sk.anniversary_years > 0 {
        assert!(
            families.contains(&sk.anniversary_beat_family),
            "campaign_skeleton anniversary_beat_family '{}' has no events",
            sk.anniversary_beat_family
        );
    }
    // Content-depth round 18: the succession beat (a sitting leader dying in
    // office) needs a family with events when set.
    if !sk.succession_beat_family.is_empty() {
        assert!(
            families.contains(&sk.succession_beat_family),
            "campaign_skeleton succession_beat_family '{}' has no events",
            sk.succession_beat_family
        );
    }
    // Content-depth round 19: the long-reign beat needs a family with events
    // when switched on.
    if sk.long_reign_years > 0 && !sk.long_reign_beat_family.is_empty() {
        assert!(
            families.contains(&sk.long_reign_beat_family),
            "campaign_skeleton long_reign_beat_family '{}' has no events",
            sk.long_reign_beat_family
        );
    }
    // Content-depth round 20: the dynasty-crisis beat needs a family with events
    // when switched on.
    if sk.dynasty_crisis_size > 0 && !sk.dynasty_crisis_beat_family.is_empty() {
        assert!(
            families.contains(&sk.dynasty_crisis_beat_family),
            "campaign_skeleton dynasty_crisis_beat_family '{}' has no events",
            sk.dynasty_crisis_beat_family
        );
    }
    // Content-depth campaign-skeleton round 21: the mid-voyage (deep-middle) beat
    // needs a family with events when switched on.
    if !sk.midvoyage_beat_family.is_empty() {
        assert!(
            families.contains(&sk.midvoyage_beat_family),
            "campaign_skeleton midvoyage_beat_family '{}' has no events",
            sk.midvoyage_beat_family
        );
    }
    // Content-depth campaign-skeleton round 22: the founding-era beat needs a family
    // with events when switched on, and an early year to fire at.
    if !sk.founding_beat_family.is_empty() {
        assert!(
            families.contains(&sk.founding_beat_family),
            "campaign_skeleton founding_beat_family '{}' has no events",
            sk.founding_beat_family
        );
        assert!(
            sk.founding_beat_year > 0,
            "the founding beat needs an early year to fire at"
        );
    }
    // Content-depth campaign-skeleton round 23: the hull-collapse beat needs a family
    // with events and a red line in (0,1) when switched on, and — like the subsystem
    // collapse beat — a `hull_below`-gated event to surface so the reckoning lands on
    // theme.
    if !sk.hull_beat_family.is_empty() {
        assert!(
            families.contains(&sk.hull_beat_family),
            "campaign_skeleton hull_beat_family '{}' has no events",
            sk.hull_beat_family
        );
        assert!(
            sk.hull_beat_threshold > 0.0 && sk.hull_beat_threshold < 1.0,
            "hull_beat_threshold {} must be a red line inside (0, 1)",
            sk.hull_beat_threshold
        );
        assert!(
            data.events.iter().any(|(_, e)| e.hull_below.is_some()),
            "the hull-collapse beat needs a hull_below-gated event to surface"
        );
    }
    // Content-depth campaign-skeleton round 32: the hull-recovery beat needs a family with
    // events and a threshold that sits *above* the collapse red line (hysteresis) and no higher
    // than a whole hull — a rebuild, not a wobble over the line.
    if !sk.hull_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.hull_recovery_beat_family),
            "campaign_skeleton hull_recovery_beat_family '{}' has no events",
            sk.hull_recovery_beat_family
        );
        assert!(
            sk.hull_recovery_beat_threshold > sk.hull_beat_threshold
                && sk.hull_recovery_beat_threshold <= 1.0,
            "hull_recovery_beat_threshold {} must sit above the collapse red line {}",
            sk.hull_recovery_beat_threshold,
            sk.hull_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 24: the air-collapse beat, the same shape,
    // needs a family with events, a red line in (0,1), and a life_support_below event.
    if !sk.air_beat_family.is_empty() {
        assert!(
            families.contains(&sk.air_beat_family),
            "campaign_skeleton air_beat_family '{}' has no events",
            sk.air_beat_family
        );
        assert!(
            sk.air_beat_threshold > 0.0 && sk.air_beat_threshold < 1.0,
            "air_beat_threshold {} must be a red line inside (0, 1)",
            sk.air_beat_threshold
        );
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.life_support_below.is_some()),
            "the air-collapse beat needs a life_support_below-gated event to surface"
        );
    }
    // Content-depth campaign-skeleton round 33: the air-recovery beat, the atmosphere twin of
    // the hull-recovery guard — a family with events and a threshold that sits *above* the
    // collapse red line (hysteresis) and no higher than a whole plant (an overhaul, not a wobble).
    if !sk.air_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.air_recovery_beat_family),
            "campaign_skeleton air_recovery_beat_family '{}' has no events",
            sk.air_recovery_beat_family
        );
        assert!(
            sk.air_recovery_beat_threshold > sk.air_beat_threshold
                && sk.air_recovery_beat_threshold <= 1.0,
            "air_recovery_beat_threshold {} must sit above the collapse red line {}",
            sk.air_recovery_beat_threshold,
            sk.air_beat_threshold
        );
    }
    // Content-depth campaign-skeleton round 25: the becalmed (mobility) beat needs a
    // family with events and a sustained-stall year count when switched on.
    if !sk.becalmed_beat_family.is_empty() {
        assert!(
            families.contains(&sk.becalmed_beat_family),
            "campaign_skeleton becalmed_beat_family '{}' has no events",
            sk.becalmed_beat_family
        );
        assert!(
            sk.becalmed_beat_years > 0,
            "the becalmed beat needs a sustained-stall threshold"
        );
    }
    // Content-depth campaign-skeleton round 34: the becalmed-recovery beat needs a family with
    // events (it has no threshold — the recovery fires when the stall counter returns to 0).
    if !sk.becalmed_recovery_beat_family.is_empty() {
        assert!(
            families.contains(&sk.becalmed_recovery_beat_family),
            "campaign_skeleton becalmed_recovery_beat_family '{}' has no events",
            sk.becalmed_recovery_beat_family
        );
    }
    // Content-depth campaign-skeleton round 26: the adaptation-divergence beat, the high-side
    // crew-body twin, needs a family with events, a red line in (0,1), and an
    // adaptation_above-gated event to surface the reckoning on theme.
    if !sk.divergence_beat_family.is_empty() {
        assert!(
            families.contains(&sk.divergence_beat_family),
            "campaign_skeleton divergence_beat_family '{}' has no events",
            sk.divergence_beat_family
        );
        assert!(
            sk.divergence_beat_threshold > 0.0 && sk.divergence_beat_threshold < 1.0,
            "divergence_beat_threshold {} must be a red line inside (0, 1)",
            sk.divergence_beat_threshold
        );
        assert!(
            data.events
                .iter()
                .any(|(_, e)| e.adaptation_above.is_some()),
            "the adaptation-divergence beat needs an adaptation_above-gated event to surface"
        );
    }
    // Content-depth campaign-skeleton round 27: the cultural-divergence beat, the cultural
    // twin, needs a family with events, a red line in (0,1) set above the top drift_beats
    // milestone (so it is terminal, not another rung), and a high-`min_cultural_drift` event
    // to surface the reckoning on theme.
    if !sk.cultural_divergence_beat_family.is_empty() {
        assert!(
            families.contains(&sk.cultural_divergence_beat_family),
            "campaign_skeleton cultural_divergence_beat_family '{}' has no events",
            sk.cultural_divergence_beat_family
        );
        assert!(
            sk.cultural_divergence_beat_threshold > 0.0
                && sk.cultural_divergence_beat_threshold < 1.0,
            "cultural_divergence_beat_threshold {} must be a red line inside (0, 1)",
            sk.cultural_divergence_beat_threshold
        );
        assert!(
            sk.drift_beats
                .iter()
                .all(|t| *t <= sk.cultural_divergence_beat_threshold),
            "the cultural-divergence red line must sit at or above every drift_beats milestone"
        );
        assert!(
            data.events.iter().any(|(_, e)| e.min_cultural_drift >= 0.8),
            "the cultural-divergence beat needs a high-min_cultural_drift event to surface"
        );
    }
}
