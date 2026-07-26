//! What the modules are worth at the year boundary: decay and the craft
//! handed down, and every factor a module's condition bends.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::effective_severity;

/// Yearly subsystem condition decay (W5), eased by the same maintained/relief
/// `wear` factor the hull uses.
pub fn decay_subsystems(sim: &mut SimState, data: &GameData, wear: f32) {
    // Keystone coupling (content-depth round 7): the engineering bay is where the
    // ship mends itself, so its condition scales every *other* module's decay —
    // a sound bay holds the whole ship together, a failing one lets it all rot.
    let swing = data.config.subsystems.engineering_decay_swing;
    let eng_condition = sim
        .subsystems
        .get("engineering_bay")
        .map_or(0.5, |s| s.condition);
    let keystone_mult = (1.0 + swing * (0.5 - eng_condition)).max(0.0);

    // Tender-approval coupling (content-depth factions round 12): the aboard people
    // that tends a module modulates its decay by their mood — devotion keeps it
    // sharp, resentment lets it slide — closing the neglect → sour → rot spiral.
    let tender_scale = data.config.subsystems.tender_approval_decay_scale;
    // Knowledge-upkeep coupling (content-depth subsystems round 33): a module the crew has
    // mastered decays slower, its faults caught early and patched cleverly.
    let knowledge_reduction = data.config.subsystems.knowledge_decay_reduction;

    for id in GameData::sorted_ids(&data.subsystems) {
        let Some(def) = data.subsystems.get(&id) else {
            continue;
        };
        // Engineering decays at its own rate; the bay is the source of the
        // keystone coupling, not subject to it.
        let mut mult = if id == "engineering_bay" {
            1.0
        } else {
            keystone_mult
        };
        if tender_scale != 0.0 {
            if let Some(approval) = sim.tender_approval(data, &id) {
                mult *= (1.0 + tender_scale * (0.5 - approval)).max(0.0);
            }
        }
        // The crew's craft with the machine itself slows its rot — scaled by the module's own
        // knowledge, kept above 0 so mastery slows the decay but never stops it.
        if knowledge_reduction != 0.0 {
            let knowledge = sim.subsystems.get(&id).map_or(0.0, |s| s.knowledge);
            mult *= (1.0 - knowledge_reduction * knowledge).max(0.0);
        }
        let decay = def.decay_per_year * mult;
        if let Some(state) = sim.subsystems.get_mut(&id) {
            state.condition = (state.condition - decay * wear).max(0.0);
        }
    }
}

/// Generation-boundary knowledge change (W5): knowledge dies with the people
/// (`-knowledge_decay_per_generation`) but the education subsystem transmits it
/// forward (`education_tier × education_transmission_per_tier`). Clamped 0-1.
pub fn transmit_knowledge(sim: &mut SimState, data: &GameData) {
    let cfg = &data.config.subsystems;
    let education = sim.subsystems.get("education_culture");
    let education_tier = education.map(|s| s.tier).unwrap_or(0);
    // Education is the knowledge keystone (content-depth subsystems round 13): a
    // well-kept archive transmits the founding craft forward in full, a crumbling
    // one loses more of it each generation. Penalty-below-full keeps the baseline.
    let education_condition = education.map_or(1.0, |s| s.condition);
    let transmission_factor =
        (1.0 - cfg.education_transmission_condition_penalty * (1.0 - education_condition)).max(0.0);
    let delta = -cfg.knowledge_decay_per_generation
        + education_tier as f32 * cfg.education_transmission_per_tier * transmission_factor;
    for id in GameData::sorted_ids(&data.subsystems) {
        if let Some(state) = sim.subsystems.get_mut(&id) {
            state.knowledge = (state.knowledge + delta).clamp(0.0, 1.0);
        }
    }
}

/// Extra food-production fraction from the agriculture subsystem (W5):
/// `tier × agriculture_food_bonus_per_tier`.
pub fn agriculture_food_bonus(sim: &SimState, data: &GameData) -> f32 {
    let tier = sim
        .subsystems
        .get("agriculture")
        .map(|s| s.tier)
        .unwrap_or(0);
    tier as f32 * data.config.subsystems.agriculture_food_bonus_per_tier
}

/// Food-yield multiplier from the agriculture bay's *condition* (content-depth
/// subsystems round 12): `1 - penalty·(1 - condition)`, clamped ≥ 0. A pristine
/// farm (condition 1.0) yields 1.0 — the untouched baseline — while a degraded one
/// feeds proportionally fewer, so keeping the hydroponics in repair pays back every
/// year rather than only staving off a breakdown. A missing bay counts as neutral.
pub fn agriculture_condition_food_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.agriculture_condition_food_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("agriculture")
        .map_or(1.0, |s| s.condition);
    (1.0 - penalty * (1.0 - condition)).max(0.0)
}

/// Crew lost this year to a life-support/habitat plant that cannot sustain everyone
/// (content-depth subsystems round 15, provisioning round 15): the module's most
/// fundamental effect. The plant needs *both* repair and power — so the effective
/// condition is the worse of its physical state and the grid's power availability,
/// and a sound plant with an empty grid kills as surely as a broken one with full
/// power. Above the failure threshold it sustains everyone (0 loss); below it, a
/// yearly attrition scaled from 0 at the threshold to `mortality × population` at
/// zero. Floored, so a barely-failing plant on a small crew may cost none.
pub fn life_support_mortality_loss(sim: &SimState, data: &GameData) -> u32 {
    let cfg = &data.config.subsystems;
    let threshold = cfg.life_support_failure_threshold;
    if threshold <= 0.0 || cfg.life_support_failure_mortality <= 0.0 {
        return 0;
    }
    let plant = sim
        .subsystems
        .get("life_support_habitat")
        .map_or(1.0, |s| s.condition);
    // Power starvation (provisioning round 15): a scrubber array with no current to
    // run it is a dead plant, whatever its repair. Below the critical grid level the
    // effective condition falls with the energy store; at or above it, full power.
    let power_avail = if cfg.life_support_energy_critical <= 0 {
        1.0
    } else {
        (sim.resources.energy as f32 / cfg.life_support_energy_critical as f32).clamp(0.0, 1.0)
    };
    // The green decks are the ship's lungs (content-depth subsystems round 17): a
    // living agriculture biosphere scrubs air the mechanical plant would otherwise
    // carry alone, so a well-kept farm supplements the plant's effective condition —
    // real slack against a failing plant, though (capped below the threshold) never a
    // wholesale replacement for it.
    let bio = cfg.agriculture_life_support_contribution
        * sim
            .subsystems
            .get("agriculture")
            .map_or(1.0, |s| s.condition);
    let condition = (plant.min(power_avail) + bio).min(1.0);
    if condition >= threshold {
        return 0;
    }
    let severity = ((threshold - condition) / threshold).clamp(0.0, 1.0);
    let fraction = cfg.life_support_failure_mortality * severity;
    // A serving infirmary fights to keep the asphyxiating alive (content-depth subsystems round
    // 31): the medical bay's condition mitigates the failing-air deaths — the third death source
    // its craft covers, after age (round 18) and famine (round 9). Kept below full relief so even
    // a perfect bay only saves some; it cannot make air out of nothing.
    let medical = sim
        .subsystems
        .get("medical_bay")
        .map_or(0.0, |s| s.condition);
    let relief = (1.0 - cfg.medical_life_support_relief * medical).max(0.0);
    (sim.population.count as f32 * fraction * relief) as u32
}

/// Fraction by which the life-support/habitat subsystem slows life-support
/// decay (W5): its current tier's `severity_reduction × condition`.
pub fn life_support_decay_reduction(sim: &SimState, data: &GameData) -> f32 {
    let Some(def) = data.subsystems.get("life_support_habitat") else {
        return 0.0;
    };
    let Some(state) = sim.subsystems.get("life_support_habitat") else {
        return 0.0;
    };
    effective_severity(def, state)
}

/// Fraction of famine losses the medical bay itself prevents (content-depth
/// subsystems round 9): a bay in good repair keeps more of the starving alive.
/// Scales by *condition* — upkeep finally buys output, not just the absence of a
/// breakdown — and stacks with the serving medic (the caller caps the total).
pub fn medical_famine_relief(sim: &SimState, data: &GameData) -> f32 {
    let condition = sim
        .subsystems
        .get("medical_bay")
        .map_or(0.0, |s| s.condition);
    condition * data.config.subsystems.medical_famine_relief_per_condition
}

/// Multiplier on the per-travel-month fuel burn from the engineering bay's state
/// (content-depth subsystems round 20): a sound bay tunes the drive to burn clean
/// (factor 1.0), a failing one burns rich and wastes reaction mass
/// (`1 + engineering_fuel_burn_penalty·(1 - condition)`). Penalty-below-full, so a
/// pristine bay is the baseline; 1.0 when the coupling is off or the module is gone.
pub fn engineering_fuel_burn_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.engineering_fuel_burn_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("engineering_bay")
        .map_or(1.0, |s| s.condition);
    (1.0 + penalty * (1.0 - condition)).max(1.0)
}

/// Fraction of the drive's fuel scooping the engineering bay's condition lets through
/// (content-depth subsystems round 30): the *production* side of the it20 fuel coupling, whose
/// *consumption* side (`engineering_fuel_burn_factor`) the bay already governs. The bay maintains
/// the drive, so it keeps the scoops and the reaction-mass plant efficient too — a sound bay
/// regenerates fuel at the full rate (factor 1.0), a rotting one fouls its own intakes and scoops
/// less (`1 - engineering_fuel_regen_penalty·(1 - condition)`, floored at 0). This makes the
/// engineering→fuel coupling two-sided: neglect the bay and the ship both *burns more* (round 20)
/// and *scoops less* (this), tightening the it25 becalming spiral from both ends. 1.0 (inert) at
/// full condition, when the penalty is 0, or when the bay is gone.
pub fn engineering_fuel_regen_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.engineering_fuel_regen_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("engineering_bay")
        .map_or(1.0, |s| s.condition);
    (1.0 - penalty * (1.0 - condition)).max(0.0)
}

/// Multiplier on the ship's yearly *hull* wear from the engineering bay's condition
/// (content-depth subsystems round 24): the engineering bay is where the ship is
/// mended, so it should keep not only the modules (the it62 decay keystone) but the
/// *hull* itself in repair — the welders, the fabricators, the crews who work the frame.
/// A sound bay holds the hull at its baseline wear (factor 1.0), a failing one lets the
/// frame rot faster (`1 + engineering_hull_decay_penalty·(1 - condition)`). Penalty-
/// below-full, floored at 1.0 so a good bay never makes the hull immortal — it only
/// keeps the normal rate — and 1.0 when the coupling is off or the module is gone.
pub fn engineering_hull_decay_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.engineering_hull_decay_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("engineering_bay")
        .map_or(1.0, |s| s.condition);
    (1.0 + penalty * (1.0 - condition)).max(1.0)
}

/// Fraction of the full fabrication yield a given engineering bay can actually turn out
/// (content-depth subsystems round 26): the engineering bay *is* the fabrication hall —
/// its top tier "remanufactures nearly anything the ship is made of" — yet the it21
/// surplus-energy fabrication run (spare watts + ore → spare parts) took a flat yield no
/// matter the bay's state. This couples the two: a sharp bay fabricates the full run, a
/// neglected one turns out less, a wrecked one only what hands and improvised tools can
/// manage. Scales `1 - penalty·(1 - condition)`, floored at 0 — the condition→output
/// coupling the food module already has (`agriculture_condition_food_factor`), now on the
/// ship's own manufacturing. 1.0 (inert) when the penalty is 0 or the bay is absent.
pub fn engineering_fabrication_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.engineering_fabrication_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("engineering_bay")
        .map_or(1.0, |s| s.condition);
    (1.0 - penalty * (1.0 - condition)).max(0.0)
}

/// Fraction by which the medical bay lowers each character's monthly age-based
/// death chance (content-depth subsystems round 18 — the first subsystem coupling
/// to the real-time-loop mortality system). Scales by *condition*, so keeping the
/// infirmary sharp keeps the crew alive longer; clamped just below 1 so a bay can
/// never make anyone immortal (and never applies at the hard age cap). 0 when the
/// coupling is disabled or the bay is gone.
pub fn medical_mortality_relief(sim: &SimState, data: &GameData) -> f32 {
    let condition = sim
        .subsystems
        .get("medical_bay")
        .map_or(0.0, |s| s.condition);
    (condition
        * data
            .config
            .subsystems
            .medical_mortality_relief_per_condition)
        .clamp(0.0, 0.9)
}

/// Yearly unity recovery from a well-kept security/justice system (content-depth
/// subsystems round 9): a functioning corps steadies a fractious ship. Scales by
/// *condition* and, like the security chief, only helps a ship still below the
/// crew recovery ceiling. Stacks with the chief.
pub fn security_unity_recovery(sim: &SimState, data: &GameData) -> f32 {
    if sim.population.unity >= data.config.crew.unity_recovery_ceiling {
        return 0.0;
    }
    let condition = sim.subsystems.get("security").map_or(0.0, |s| s.condition);
    condition * data.config.subsystems.security_unity_recovery_per_condition
}

/// Yearly *stability* recovery from a well-kept security/justice corps (content-depth
/// subsystems round 16): the corps keeping the ship's institutions functioning — the
/// governance twin of `security_unity_recovery`, and the first maintenance-driven
/// counterweight the it102 stability stat has. Scales by condition; only steadies a
/// ship still below the ceiling (the corps does not build perfect order from nothing).
pub fn security_stability_recovery(sim: &SimState, data: &GameData) -> f32 {
    let cfg = &data.config.subsystems;
    if cfg.security_stability_recovery_per_condition <= 0.0
        || sim.population.stability >= cfg.security_stability_recovery_ceiling
    {
        return 0.0;
    }
    let condition = sim.subsystems.get("security").map_or(0.0, |s| s.condition);
    condition * cfg.security_stability_recovery_per_condition
}

/// Fraction of the ideology-spread governance drain a given security corps lets through
/// (content-depth subsystems round 28): the peacekeeping corps' *third* role beside its two
/// recoveries, and the one the it18 factions comment had long promised but never wired — a
/// corps whose whole craft is mediating a fractious ship *directly* dampens how much the it18
/// ideological division actually erodes `stability`. Distinct from `security_stability_recovery`
/// (which lifts a *fallen* stability back toward a ceiling): this reduces the *drain itself* at
/// its source, so a divided ship with a strong corps governs better not by recovering after the
/// strain but by feeling less of it. Returns `1 - condition·ideology_spread_security_relief`
/// (the multiplier applied to the drain), floored at 0 — a corps can soften the strain of a
/// split polity but, with the relief kept below 1, never wholly cancel it. 1.0 (inert) when the
/// relief is 0 or the corps is gone.
pub fn security_spread_relief_factor(sim: &SimState, data: &GameData) -> f32 {
    let relief = data.config.subsystems.ideology_spread_security_relief;
    if relief == 0.0 {
        return 1.0;
    }
    let condition = sim.subsystems.get("security").map_or(0.0, |s| s.condition);
    (1.0 - condition * relief).max(0.0)
}

/// Fraction of a training cohort's knowledge gain a given academy actually imparts
/// (content-depth subsystems round 27): the education/culture module *is* the ship's schools,
/// so its condition scales how much a deliberate `train_subsystem_knowledge` cohort learns —
/// a true academy trains new crews to the full craft, a crumbling one (rote lessons, lost
/// method, no masters to apprentice under) teaches them only a fraction. This gives education
/// a third active role beside its two keystones — the generational `transmit_knowledge` and
/// the it10 archive drift-resistance — now touching *deliberate* craft-building, not only what
/// passes on by default. Scales `1 - education_training_penalty·(1 - condition)`; kept above 0
/// by a penalty below 1, so even a wrecked academy can still bootstrap itself back (no repair
/// deadlock). 1.0 (inert) when the penalty is 0 or the academy is absent.
pub fn education_training_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.education_training_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("education_culture")
        .map_or(1.0, |s| s.condition);
    (1.0 - penalty * (1.0 - condition)).max(0.0)
}

/// Signed yearly morale shift from the state of the habitat (content-depth
/// subsystems round 11): the life-support/habitat is where the people live, so a
/// home kept above the midpoint lifts spirits and one let to fail depresses them —
/// `swing * (condition - 0.5)`. 0 when the module is gone (neutral, no home to
/// lift or lose).
pub fn habitat_morale_effect(sim: &SimState, data: &GameData) -> f32 {
    let swing = data.config.subsystems.habitat_morale_swing;
    if swing == 0.0 {
        return 0.0;
    }
    match sim.subsystems.get("life_support_habitat") {
        Some(s) => swing * (s.condition - 0.5),
        None => 0.0,
    }
}

/// Signed yearly morale shift from the state of the ship's *cultural* life
/// (content-depth subsystems round 22): the education/culture module is the ship's
/// schools, theatres, festival spaces, and living memory — the pillar of morale the
/// habitat (the physical home) and the larder (nourishment) do not cover. A crew can
/// be warm and well-fed and still grim if its cultural life has hollowed out (schools
/// teaching rote, the arts forgotten, the year's festivals lapsed); a vivid one lifts
/// spirits the way a good home does. `swing * (condition - 0.5)`, the cultural twin of
/// `habitat_morale_effect`, reading education's *condition* (its facilities functioning)
/// — distinct from its *knowledge* (the founding remembered), which resists drift (it).
/// 0 when the module is gone.
pub fn education_morale_effect(sim: &SimState, data: &GameData) -> f32 {
    let swing = data.config.subsystems.education_morale_swing;
    if swing == 0.0 {
        return 0.0;
    }
    match sim.subsystems.get("education_culture") {
        Some(s) => swing * (s.condition - 0.5),
        None => 0.0,
    }
}

/// Multiplier on the dynasty's yearly renewal from the habitat's condition
/// (content-depth subsystems round 19): a home kept sound raises the young on
/// schedule (factor 1.0), a failing one sees fewer come of age
/// (`1 - habitat_renewal_penalty·(1 - condition)`). Penalty-below-full, so a
/// pristine habitat is the baseline; 1.0 when the coupling is off or the module is
/// gone (renewal unchanged).
pub fn habitat_renewal_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.habitat_renewal_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("life_support_habitat")
        .map_or(1.0, |s| s.condition);
    (1.0 - penalty * (1.0 - condition)).clamp(0.0, 1.0)
}

/// Multiplier on the dynasty's yearly renewal from the *medical bay's* condition
/// (content-depth subsystems round 23): where the habitat is where children are
/// *raised* (it19), the infirmary is what keeps them *alive* to grow up — prenatal and
/// infant care, the fevers of childhood caught in time. A sound bay brings the cohort
/// up whole (factor 1.0), a failing one loses more of the young before their majority
/// (`1 - medical_renewal_penalty·(1 - condition)`). Penalty-below-full, so a pristine
/// bay is the baseline; stacks with the habitat factor (housing × healthcare), and
/// distinct from the it medical *death* relief, which keeps the grown alive. 1.0 when
/// the coupling is off or the module is gone.
pub fn medical_renewal_factor(sim: &SimState, data: &GameData) -> f32 {
    let penalty = data.config.subsystems.medical_renewal_penalty;
    if penalty == 0.0 {
        return 1.0;
    }
    let condition = sim
        .subsystems
        .get("medical_bay")
        .map_or(1.0, |s| s.condition);
    (1.0 - penalty * (1.0 - condition)).clamp(0.0, 1.0)
}
