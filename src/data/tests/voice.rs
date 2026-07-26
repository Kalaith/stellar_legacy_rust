//! The authored voice: every pool a narrator can draw from is stocked,
//! rotates without repeating, and substitutes the names it promises.

use super::*;

#[test]
fn flavor_lines_rotate_deterministically_and_substitute_name() {
    let pool = vec!["A {name}".to_string(), "B {name}".to_string()];
    // Rotates by index, wraps, and substitutes — no RNG, so a seed replays.
    assert_eq!(
        FlavorConfig::line_with_name(&pool, 0, "Vale").unwrap(),
        "A Vale"
    );
    assert_eq!(
        FlavorConfig::line_with_name(&pool, 1, "Vale").unwrap(),
        "B Vale"
    );
    assert_eq!(
        FlavorConfig::line_with_name(&pool, 2, "Vale").unwrap(),
        "A Vale"
    );
    assert!(FlavorConfig::line_with_name(&[], 0, "Vale").is_none());
}

#[test]
fn homecoming_lines_are_authored_for_every_success_level_and_substitute() {
    // Content-depth voice round 4: every mission outcome the game can log
    // must have homecoming prose, indexed deterministically and with the
    // voyage's length/generation woven in.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for level in ["complete", "partial", "pyrrhic", "failure"] {
        let line = flavor
            .homecoming_line(level, 0, 450, 17)
            .unwrap_or_else(|| panic!("no homecoming prose for '{level}'"));
        assert!(
            line.contains("450") || line.contains("17"),
            "'{level}' homecoming should weave in the voyage's span: {line}"
        );
    }
    // Deterministic rotation by the index, and an unknown level is None.
    let a = flavor.homecoming_line("complete", 0, 300, 10);
    let b = flavor.homecoming_line("complete", 0, 300, 10);
    assert_eq!(a, b, "same index replays the same line");
    assert!(flavor.homecoming_line("triumphant", 0, 300, 10).is_none());
}

#[test]
fn generational_turnover_voice_is_authored_and_varies() {
    // Content-depth voice round 5: the crew-retirement line fires several
    // times a generation, so its pool must have real variety (not a
    // repetition tell); the extinction ending must have authored prose too.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    assert!(
        flavor.retirement.len() >= 4,
        "the retirement pool needs variety — it fires several times a generation"
    );
    assert!(
        !flavor.extinction.is_empty(),
        "the line-ends ending needs prose"
    );
    // Consecutive retirements (the same generation) draw different lines.
    let a = FlavorConfig::line_with_name(&flavor.retirement, 0, "Vale").unwrap();
    let b = FlavorConfig::line_with_name(&flavor.retirement, 1, "Vale").unwrap();
    assert_ne!(
        a, b,
        "two stand-downs in one generation must read differently"
    );
    assert!(a.contains("Vale"), "the retiring holder's name is woven in");
}

#[test]
fn crew_appointment_and_training_voice_is_authored_and_varies() {
    // Content-depth voice round 7: the appointment line (the positive twin of
    // retirement) and the training line both fire repeatedly as a roster is
    // re-crewed over the centuries, so both need pool variety and must weave
    // in the officer's name and human post — not the raw archetype id.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    assert!(
        flavor.appointment.len() >= 4,
        "the appointment pool needs variety — posts turn over all voyage"
    );
    assert!(
        flavor.training.len() >= 3,
        "the training pool needs variety — training is a repeatable verb"
    );
    // Two appointments in one drydock draw different lines, and both weave in
    // the officer's name and the human post name.
    let a = FlavorConfig::line_with_name_post(&flavor.appointment, 0, "Vale", "Chief Engineer")
        .unwrap();
    let b = FlavorConfig::line_with_name_post(&flavor.appointment, 1, "Vale", "Chief Engineer")
        .unwrap();
    assert_ne!(a, b, "two appointments must read differently");
    assert!(
        a.contains("Vale") && a.contains("Chief Engineer"),
        "the appointee's name and human post are woven in"
    );
    // The training pool carries the skill placeholder.
    assert!(
        flavor.training.iter().any(|s| s.contains("{skill}")),
        "training lines surface the new skill"
    );
}

#[test]
fn faction_mood_voice_is_authored_and_names_the_people() {
    // Content-depth voice round 8: the approval meter's voice. A people
    // crossing into restlessness or contentment gets a pooled line, so both
    // pools need variety and must weave in the people's name.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for (pool, label) in [
        (&flavor.faction_souring, "souring"),
        (&flavor.faction_warming, "warming"),
    ] {
        assert!(pool.len() >= 3, "the faction {label} pool needs variety");
        assert!(
            pool.iter().all(|s| s.contains("{name}")),
            "every faction {label} line must name the people"
        );
        let a = FlavorConfig::line_with_name(pool, 0, "the Keepers").unwrap();
        let b = FlavorConfig::line_with_name(pool, 1, "the Keepers").unwrap();
        assert_ne!(a, b, "consecutive {label} lines must differ");
        assert!(
            a.contains("the Keepers"),
            "the {label} line names the people"
        );
    }
}

#[test]
fn ship_mood_voice_is_authored_and_varies() {
    // Content-depth voice round 11: the ship's collective morale crossing into
    // a grim or a buoyant band draws a pooled ambient line — the ship-wide twin
    // of the faction-mood voice. No name to weave, but both pools need variety.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for (pool, label) in [
        (&flavor.ship_mood_darkening, "darkening"),
        (&flavor.ship_mood_lifting, "lifting"),
    ] {
        assert!(pool.len() >= 3, "the ship-mood {label} pool needs variety");
        let a = FlavorConfig::line_with_name(pool, 0, "").unwrap();
        let b = FlavorConfig::line_with_name(pool, 1, "").unwrap();
        assert_ne!(a, b, "consecutive ship-mood {label} lines must differ");
    }
}

#[test]
fn subsystem_maintenance_voice_is_authored_and_names_the_module() {
    // Content-depth voice round 9: the field-repair and knowledge-training
    // verbs fire repeatedly across a voyage, so both pools need variety and
    // must weave in the module name.
    let data = GameData::load().unwrap();
    let flavor = &data.config.flavor;
    for (pool, label) in [
        (&flavor.subsystem_repair, "repair"),
        (&flavor.subsystem_training, "training"),
    ] {
        assert!(pool.len() >= 3, "the subsystem {label} pool needs variety");
        assert!(
            pool.iter().all(|s| s.contains("{name}")),
            "every subsystem {label} line must name the module"
        );
        let a = FlavorConfig::line_with_name(pool, 0, "engineering bay").unwrap();
        let b = FlavorConfig::line_with_name(pool, 1, "engineering bay").unwrap();
        assert_ne!(a, b, "consecutive {label} lines must differ");
        assert!(a.contains("engineering bay"), "the {label} line names it");
    }
}

/// Voice: the generational-turnover pools must all carry lines.
#[test]
fn every_generational_flavor_pool_is_stocked() {
    let data = GameData::load().unwrap();
    // Content-depth voice: every generational-flavor pool must be non-empty
    // (or a generation turns over in silence) and carry its placeholder.
    let fl = &data.config.flavor;
    assert!(
        fl.obituary.iter().any(|s| s.contains("{name}")),
        "obituary flavor needs a {{name}} line"
    );
    assert!(
        fl.succession.iter().any(|s| s.contains("{name}")),
        "succession flavor needs a {{name}} line"
    );
    assert!(
        !fl.coming_of_age.is_empty(),
        "coming_of_age flavor must not be empty"
    );
    // Content-depth voice round 19: the two high-frequency pooled lines must
    // carry their substitution slot, or the mark/summons reads with a literal.
    assert!(
        fl.milestone.is_empty() || fl.milestone.iter().all(|s| s.contains("{milestone}")),
        "every milestone flavor line needs its {{milestone}} slot"
    );
    assert!(
        fl.council_summons.is_empty() || fl.council_summons.iter().all(|s| s.contains("{title}")),
        "every council_summons flavor line needs its {{title}} slot"
    );
    // Content-depth voice round 24: the pooled disaster-death and failing-air lines
    // carry their substitution slots, or a loss reads with a literal placeholder.
    assert!(
        fl.event_loss_officer
            .iter()
            .all(|s| s.contains("{name}") && s.contains("{post}")),
        "every event_loss_officer line needs its {{name}} and {{post}} slots"
    );
    assert!(
        fl.event_loss_member.iter().all(|s| s.contains("{name}")),
        "every event_loss_member line needs its {{name}} slot"
    );
    assert!(
        fl.life_support_loss.iter().all(|s| s.contains("{losses}")),
        "every life_support_loss line needs its {{losses}} slot"
    );
}

/// Voice: a band narrator that is switched on needs both its rising and
/// falling pools stocked, or a crossing would speak an empty line.
#[test]
fn every_band_voice_is_stocked_on_both_sides() {
    let data = GameData::load().unwrap();
    let fl = &data.config.flavor;
    // Content-depth voice round 2: if ambient flavor is switched on, it needs
    // lines to draw from.
    if fl.ambient_gap_years > 0 {
        assert!(
            !fl.ambient.is_empty(),
            "ambient_gap_years is set but the ambient pool is empty"
        );
    }
    // Content-depth voice round 20: if the loyalty voice is switched on, both
    // bands need lines to draw from, and the thresholds must order (low < high,
    // both inside 0..1) or the crossing logic is nonsense.
    if fl.loyalty_voice_high > 0.0 {
        assert!(
            !fl.loyalty_guttering.is_empty() && !fl.loyalty_bright.is_empty(),
            "loyalty voice is enabled but a band pool is empty"
        );
        assert!(
            fl.loyalty_voice_low > 0.0 && fl.loyalty_voice_low < fl.loyalty_voice_high,
            "loyalty voice thresholds must order: 0 < low ({}) < high ({})",
            fl.loyalty_voice_low,
            fl.loyalty_voice_high
        );
        assert!(
            fl.loyalty_voice_high < 1.0,
            "loyalty_voice_high {} must be below 1.0 to be reachable",
            fl.loyalty_voice_high
        );
    }
    // Content-depth voice round 25: the adaptation (shipborn) voice, the same shape.
    if fl.adaptation_voice_high > 0.0 {
        assert!(
            !fl.crew_shipborn.is_empty() && !fl.crew_baseline.is_empty(),
            "adaptation voice is enabled but a band pool is empty"
        );
        assert!(
            fl.adaptation_voice_low > 0.0 && fl.adaptation_voice_low < fl.adaptation_voice_high,
            "adaptation voice thresholds must order: 0 < low ({}) < high ({})",
            fl.adaptation_voice_low,
            fl.adaptation_voice_high
        );
    }
    // Content-depth voice round 26: the cultural-drift (new-people) voice, the same shape
    // as the adaptation voice — its cultural twin.
    if fl.drift_voice_high > 0.0 {
        assert!(
            !fl.culture_newfound.is_empty() && !fl.culture_founders_kept.is_empty(),
            "drift voice is enabled but a band pool is empty"
        );
        assert!(
            fl.drift_voice_low > 0.0 && fl.drift_voice_low < fl.drift_voice_high,
            "drift voice thresholds must order: 0 < low ({}) < high ({})",
            fl.drift_voice_low,
            fl.drift_voice_high
        );
    }
    // Content-depth voice round 21: likewise the unity (cohesion) voice needs both
    // band pools stocked and its thresholds ordered when it is switched on.
    if fl.unity_voice_high > 0.0 {
        assert!(
            !fl.unity_fraying.is_empty() && !fl.unity_cohering.is_empty(),
            "unity voice is enabled but a band pool is empty"
        );
        assert!(
            fl.unity_voice_low > 0.0 && fl.unity_voice_low < fl.unity_voice_high,
            "unity voice thresholds must order: 0 < low ({}) < high ({})",
            fl.unity_voice_low,
            fl.unity_voice_high
        );
    }
    // Content-depth voice round 22: and the hull (ship's body) voice, the same shape.
    if fl.hull_voice_high > 0.0 {
        assert!(
            !fl.hull_groaning.is_empty() && !fl.hull_sound.is_empty(),
            "hull voice is enabled but a band pool is empty"
        );
        assert!(
            fl.hull_voice_low > 0.0 && fl.hull_voice_low < fl.hull_voice_high,
            "hull voice thresholds must order: 0 < low ({}) < high ({})",
            fl.hull_voice_low,
            fl.hull_voice_high
        );
    }
    // Content-depth voice round 23: and the air (life-support) voice, the same shape.
    if fl.air_voice_high > 0.0 {
        assert!(
            !fl.air_stale.is_empty() && !fl.air_fresh.is_empty(),
            "air voice is enabled but a band pool is empty"
        );
        assert!(
            fl.air_voice_low > 0.0 && fl.air_voice_low < fl.air_voice_high,
            "air voice thresholds must order: 0 < low ({}) < high ({})",
            fl.air_voice_low,
            fl.air_voice_high
        );
    }
    // Content-depth voice round 27: the drive (fuel) voice, the same shape as the hull and
    // air voices — the third ship-body voice.
    if fl.fuel_voice_high > 0.0 {
        assert!(
            !fl.drive_thin.is_empty() && !fl.drive_strong.is_empty(),
            "drive voice is enabled but a band pool is empty"
        );
        assert!(
            fl.fuel_voice_low > 0.0 && fl.fuel_voice_low < fl.fuel_voice_high,
            "drive voice thresholds must order: 0 < low ({}) < high ({})",
            fl.fuel_voice_low,
            fl.fuel_voice_high
        );
    }
    // Content-depth voice round 30: the crew-size voice needs both band pools stocked, and its
    // ratios must order — a swelling line above the founding complement (> 1) and a thinning
    // one below it (0 < low < 1 < high) — so the two bands never overlap.
    if fl.crew_size_voice_high_ratio > 0.0 {
        assert!(
            !fl.crew_swelling.is_empty() && !fl.crew_thinning.is_empty(),
            "crew-size voice is enabled but a band pool is empty"
        );
        assert!(
            fl.crew_size_voice_low_ratio > 0.0
                && fl.crew_size_voice_low_ratio < 1.0
                && fl.crew_size_voice_high_ratio > 1.0,
            "crew-size voice ratios must order: 0 < low ({}) < 1 < high ({})",
            fl.crew_size_voice_low_ratio,
            fl.crew_size_voice_high_ratio
        );
    }
    // Content-depth voice round 32: the treasury voice, the same shape as the crew-size voice —
    // both band pools stocked and its ratios ordered (a flush band above the founding stake, a
    // bare one below it) when enabled, so the two bands never overlap.
    if fl.treasury_voice_high_ratio > 0.0 {
        assert!(
            !fl.treasury_flush.is_empty() && !fl.treasury_bare.is_empty(),
            "treasury voice is enabled but a band pool is empty"
        );
        assert!(
            fl.treasury_voice_low_ratio > 0.0
                && fl.treasury_voice_low_ratio < 1.0
                && fl.treasury_voice_high_ratio > 1.0,
            "treasury voice ratios must order: 0 < low ({}) < 1 < high ({})",
            fl.treasury_voice_low_ratio,
            fl.treasury_voice_high_ratio
        );
    }
    // Content-depth voice round 33: the power voice, the treasury's energy sibling — both band
    // pools stocked and its absolute energy lines ordered (0 < dark < flush) when enabled, and
    // the founding stock bracketed between them so a launched ship reads neutral, not flush.
    if fl.power_voice_high > 0 {
        assert!(
            !fl.power_flush.is_empty() && !fl.power_starved.is_empty(),
            "power voice is enabled but a band pool is empty"
        );
        assert!(
            fl.power_voice_low > 0 && fl.power_voice_low < fl.power_voice_high,
            "power voice lines must order: 0 < dark ({}) < flush ({})",
            fl.power_voice_low,
            fl.power_voice_high
        );
        assert!(
            data.config.starting_resources.energy > fl.power_voice_low
                && data.config.starting_resources.energy < fl.power_voice_high,
            "the founding energy stock {} must sit between the power lines ({}, {}) so launch reads neutral",
            data.config.starting_resources.energy,
            fl.power_voice_low,
            fl.power_voice_high
        );
    }
    // Content-depth voice round 31: the ruling-people voice, when stocked, must name the new
    // majority — every line carries the `{name}` placeholder, or the changing of the guard
    // would announce a ship passing into the hands of nobody.
    assert!(
        fl.ruling_people_change
            .iter()
            .all(|line| line.contains("{name}")),
        "every ruling_people_change line must carry the {{name}} placeholder"
    );
    // Content-depth voice round 28: the wonder reputation voice, the same shape — both band
    // pools stocked and the thresholds ordered when enabled.
    if fl.wonder_voice_high > 0.0 {
        assert!(
            !fl.wonder_famed.is_empty() && !fl.wonder_incurious.is_empty(),
            "wonder voice is enabled but a band pool is empty"
        );
        assert!(
            fl.wonder_voice_low > 0.0 && fl.wonder_voice_low < fl.wonder_voice_high,
            "wonder voice thresholds must order: 0 < low ({}) < high ({})",
            fl.wonder_voice_low,
            fl.wonder_voice_high
        );
    }
    // Content-depth voice round 29: the resolve reputation voice, the same shape — the third
    // built-trait voice, both pools stocked and the thresholds ordered when enabled.
    if fl.resolve_voice_high > 0.0 {
        assert!(
            !fl.resolve_steadfast.is_empty() && !fl.resolve_yielding.is_empty(),
            "resolve voice is enabled but a band pool is empty"
        );
        assert!(
            fl.resolve_voice_low > 0.0 && fl.resolve_voice_low < fl.resolve_voice_high,
            "resolve voice thresholds must order: 0 < low ({}) < high ({})",
            fl.resolve_voice_low,
            fl.resolve_voice_high
        );
    }
    // Content-depth voice round 6: the recurring-crisis pools need variety
    // (they fire per year the crisis lasts), and famine weaves in its toll.
    assert!(
        fl.famine.len() >= 3 && fl.famine.iter().any(|s| s.contains("{losses}")),
        "the famine pool needs variety and a {{losses}} line"
    );
    assert!(
        fl.fuel_stall.len() >= 3,
        "the fuel-stall pool needs variety"
    );
    // Content-depth voice round 3: phase-line pool keys must be real phases.
    for key in fl.phase_lines.keys() {
        assert!(
            matches!(
                key.as_str(),
                "preparation" | "travel" | "operation" | "return" | "completion"
            ),
            "flavor.phase_lines has an unknown phase key '{key}'"
        );
    }
}
