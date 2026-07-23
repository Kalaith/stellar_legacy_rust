//! Ship-subsystem services (W5): yearly decay, generational knowledge transfer,
//! the repair/upgrade/train verbs, and the event buffering each module family
//! provides. All balance comes from data; this only reads ids and applies it.

use crate::data::subsystems::SubsystemDef;
use crate::data::{GameData, PopulationDelta, ResourceDelta, ShipDelta};
use crate::state::sim::subsystems::SubsystemState;
use crate::state::sim::SimState;

/// The catalog subsystem whose `buffers_family` matches `family`, in sorted-id
/// order (deterministic). `None` for the empty family or no match.
fn buffering_def<'a>(data: &'a GameData, family: &str) -> Option<&'a SubsystemDef> {
    if family.is_empty() {
        return None;
    }
    GameData::sorted_ids(&data.subsystems)
        .into_iter()
        .find_map(|id| {
            data.subsystems
                .get(&id)
                .filter(|d| d.buffers_family == family)
        })
}

/// Effective buffer strength (0-1) a subsystem provides right now: its current
/// tier's `severity_reduction` scaled by condition. Baseline tier 0 gives 0.
fn effective_severity(def: &SubsystemDef, state: &SubsystemState) -> f32 {
    match def.tier_stats(state.tier) {
        Some(tier) => (tier.severity_reduction * state.condition).clamp(0.0, 1.0),
        None => 0.0,
    }
}

/// Roll-weight factor for an event of `family` (W5): a buffering subsystem makes
/// its family rarer, scaled by condition — `1 - (1 - weight_multiplier) × cond`.
pub fn family_weight_factor(sim: &SimState, data: &GameData, family: &str) -> f32 {
    let Some(def) = buffering_def(data, family) else {
        return 1.0;
    };
    let Some(state) = sim.subsystems.get(&def.id) else {
        return 1.0;
    };
    let Some(tier) = def.tier_stats(state.tier) else {
        return 1.0;
    };
    1.0 - (1.0 - tier.weight_multiplier) * state.condition
}

/// Scale every NEGATIVE component of an outcome's deltas by the subsystem
/// buffering `family` (W5). Positive components are untouched. Returns the
/// buffered copies to apply.
pub fn buffered_deltas(
    sim: &SimState,
    data: &GameData,
    family: &str,
    resource: ResourceDelta,
    ship: ShipDelta,
    population: PopulationDelta,
) -> (ResourceDelta, ShipDelta, PopulationDelta) {
    let factor = match buffering_def(data, family) {
        Some(def) => match sim.subsystems.get(&def.id) {
            Some(state) => 1.0 - effective_severity(def, state),
            None => 1.0,
        },
        None => 1.0,
    };
    if factor >= 1.0 {
        return (resource, ship, population);
    }
    (
        scale_resource(resource, factor),
        scale_ship(ship, factor),
        scale_population(population, factor),
    )
}

fn soften_i64(x: i64, factor: f32) -> i64 {
    if x < 0 {
        (x as f32 * factor) as i64
    } else {
        x
    }
}
fn soften_i32(x: i32, factor: f32) -> i32 {
    if x < 0 {
        (x as f32 * factor) as i32
    } else {
        x
    }
}
fn soften_f32(x: f32, factor: f32) -> f32 {
    if x < 0.0 {
        x * factor
    } else {
        x
    }
}

fn scale_resource(d: ResourceDelta, f: f32) -> ResourceDelta {
    ResourceDelta {
        credits: soften_i64(d.credits, f),
        energy: soften_i64(d.energy, f),
        minerals: soften_i64(d.minerals, f),
        food: soften_i64(d.food, f),
        influence: soften_i64(d.influence, f),
    }
}
fn scale_ship(d: ShipDelta, f: f32) -> ShipDelta {
    ShipDelta {
        hull_integrity: soften_f32(d.hull_integrity, f),
        life_support: soften_f32(d.life_support, f),
        fuel: soften_f32(d.fuel, f),
        spare_parts: soften_i32(d.spare_parts, f),
    }
}
fn scale_population(d: PopulationDelta, f: f32) -> PopulationDelta {
    PopulationDelta {
        count: soften_i32(d.count, f),
        morale: soften_f32(d.morale, f),
        unity: soften_f32(d.unity, f),
        stability: soften_f32(d.stability, f),
        legacy_loyalty: soften_f32(d.legacy_loyalty, f),
        adaptation: soften_f32(d.adaptation, f),
        cultural_drift: soften_f32(d.cultural_drift, f),
    }
}

// --- Verbs (dispatched from game/actions.rs) ---

/// Repair a subsystem (W5), underway or in port. Requires living expertise —
/// knowledge >= the subsystem's threshold — then spends parts + minerals to
/// restore condition (field ceiling underway, whole in port).
pub fn repair_subsystem(sim: &mut SimState, data: &GameData, id: &str) -> Result<(), String> {
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let knowledge = sim.subsystems.get(id).map(|s| s.knowledge).unwrap_or(0.0);
    if knowledge < def.repair_knowledge_required {
        return Err(format!(
            "No one aboard remembers how to mend the {}.",
            def.name
        ));
    }
    let current = sim.subsystems.get(id).map(|s| s.condition).unwrap_or(0.0);
    let in_port = sim.contract.is_none();
    let ceiling = if in_port {
        1.0
    } else {
        data.config.repair.field_ceiling
    };
    if current >= ceiling {
        return Err("It is already as sound as this can make it here.".to_owned());
    }
    let minerals = ResourceDelta {
        minerals: -def.repair_minerals_cost,
        ..Default::default()
    };
    if sim.ship.spare_parts < def.repair_parts_cost || !sim.resources.can_afford(&minerals) {
        return Err("Not enough spare parts or minerals to mend it.".to_owned());
    }
    sim.resources.apply(&minerals);
    sim.ship.spare_parts -= def.repair_parts_cost;
    let restored = if in_port {
        1.0
    } else {
        (current + data.config.repair.field_gain).min(ceiling)
    };
    let name = def.name.clone();
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.condition = restored;
    }
    // Data-driven so a voyage's many field repairs do not reprint one line
    // (content-depth voice round 9); indexed by the month clock, built-in fallback.
    let line = crate::data::FlavorConfig::line_with_name(
        &data.config.flavor.subsystem_repair,
        sim.month_clock as usize,
        &name,
    )
    .unwrap_or_else(|| format!("The {name} is patched back toward working order."));
    sim.push_log(line);
    Ok(())
}

/// Upgrade a subsystem one tier (W5), port only. Pays the next tier's cost.
/// Tiers cap at 3.
pub fn upgrade_subsystem(sim: &mut SimState, data: &GameData, id: &str) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("Subsystems are rebuilt in drydock, between missions.".to_owned());
    }
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let name = def.name.clone();
    let tier = sim.subsystems.get(id).map(|s| s.tier).unwrap_or(0);
    let Some(next) = def.tiers.get(tier as usize) else {
        return Err(format!("The {name} is already at its highest tier."));
    };
    let cost = ResourceDelta {
        credits: -next.cost.credits,
        energy: -next.cost.energy,
        minerals: -next.cost.minerals,
        food: -next.cost.food,
        influence: -next.cost.influence,
    };
    if !sim.resources.can_afford(&cost) {
        return Err("The treasury cannot cover that upgrade.".to_owned());
    }
    sim.resources.apply(&cost);
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.tier += 1;
    }
    // Tier-specific flavor (content-depth round 5): each module's rebuild reads
    // in its own voice; an unauthored tier falls back to the generic line so the
    // log never blanks.
    let line = if next.flavor.is_empty() {
        format!("The {name} is rebuilt stronger.")
    } else {
        next.flavor.clone()
    };
    sim.push_log(line);
    Ok(())
}

/// Train institutional knowledge for a subsystem (W5), anytime — the mid-voyage
/// recovery path when the experts have died out.
pub fn train_subsystem_knowledge(
    sim: &mut SimState,
    data: &GameData,
    id: &str,
) -> Result<(), String> {
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let cfg = &data.config.subsystems;
    let cost = ResourceDelta {
        credits: -cfg.train_cost_credits,
        ..Default::default()
    };
    if !sim.resources.can_afford(&cost) {
        return Err(format!(
            "Training a new cohort needs {} credits.",
            cfg.train_cost_credits
        ));
    }
    sim.resources.apply(&cost);
    let name = def.name.clone();
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.knowledge = (state.knowledge + cfg.train_knowledge_gain).min(1.0);
    }
    let line = crate::data::FlavorConfig::line_with_name(
        &data.config.flavor.subsystem_training,
        sim.month_clock as usize,
        &name,
    )
    .unwrap_or_else(|| format!("A new cohort trains up on the {name}."));
    sim.push_log(line);
    Ok(())
}

// --- Year-boundary tick helpers ---

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
    (sim.population.count as f32 * fraction) as u32
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sim::founding_faction_ids;

    fn campaign(seed: u64) -> (GameData, SimState) {
        let data = GameData::load().unwrap();
        let picks = founding_faction_ids(&data);
        let sim = SimState::new_campaign(&data, "preservers", seed, &picks);
        (data, sim)
    }

    #[test]
    fn a_sound_habitat_lifts_morale_and_a_failing_one_drags_it() {
        // Content-depth subsystems round 11: the habitat is where the people live,
        // so its condition moves morale — a home above the midpoint lifts spirits,
        // one below it depresses them, and a neutral one does neither.
        let (data, mut sim) = campaign(12);
        let swing = data.config.subsystems.habitat_morale_swing;
        assert!(
            swing > 0.0,
            "this test needs the habitat morale coupling enabled"
        );

        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 1.0;
        assert!(
            habitat_morale_effect(&sim, &data) > 0.0,
            "a home kept sound lifts the ship's spirits"
        );
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 0.1;
        assert!(
            habitat_morale_effect(&sim, &data) < 0.0,
            "a failing home drags the ship's spirits down"
        );
        sim.subsystems
            .get_mut("life_support_habitat")
            .unwrap()
            .condition = 0.5;
        assert_eq!(
            habitat_morale_effect(&sim, &data),
            0.0,
            "a middling home neither lifts nor drags"
        );
    }

    #[test]
    fn a_well_kept_medical_bay_and_corps_earn_their_keep_by_condition() {
        // Content-depth subsystems round 9: the two modules that only ever cost
        // the ship now pay it back, and by how well they are *kept*. A sound
        // medical bay softens famine relief above a wrecked one; a sound security
        // corps recovers more unity than a wrecked one.
        let (data, mut sim) = campaign(9);

        // Medical: relief scales with condition, so a rotted bay saves fewer.
        sim.subsystems.get_mut("medical_bay").unwrap().condition = 1.0;
        let relief_sound = medical_famine_relief(&sim, &data);
        sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.1;
        let relief_wrecked = medical_famine_relief(&sim, &data);
        assert!(
            relief_sound > relief_wrecked && relief_wrecked >= 0.0,
            "a bay in good repair keeps more of the starving alive"
        );

        // Security: recovery scales with condition, but only below the ceiling.
        sim.population.unity = 0.3;
        sim.subsystems.get_mut("security").unwrap().condition = 1.0;
        let recover_sound = security_unity_recovery(&sim, &data);
        sim.subsystems.get_mut("security").unwrap().condition = 0.1;
        let recover_wrecked = security_unity_recovery(&sim, &data);
        assert!(
            recover_sound > recover_wrecked,
            "a functioning corps steadies the ship more than a decayed one"
        );
        // Above the ceiling neither the chief nor the corps manufactures harmony.
        sim.population.unity = data.config.crew.unity_recovery_ceiling;
        sim.subsystems.get_mut("security").unwrap().condition = 1.0;
        assert_eq!(
            security_unity_recovery(&sim, &data),
            0.0,
            "a steady ship needs no steadying"
        );
    }

    #[test]
    fn a_functioning_security_corps_keeps_the_institutions_in_order() {
        // Content-depth subsystems round 16: the security corps' governance domain.
        // A sound corps recovers stability toward the ceiling, a wrecked one far
        // less, and a ship already well-ordered gets no boost.
        let data = GameData::load().unwrap();
        let cfg = &data.config.subsystems;
        assert!(
            cfg.security_stability_recovery_per_condition > 0.0,
            "this test needs the security→stability coupling enabled"
        );
        let (_, mut sim) = campaign(17);

        // A fracturing government: a sound corps steadies it more than a decayed one.
        sim.population.stability = 0.3;
        sim.subsystems.get_mut("security").unwrap().condition = 1.0;
        let recover_sound = security_stability_recovery(&sim, &data);
        sim.subsystems.get_mut("security").unwrap().condition = 0.1;
        let recover_wrecked = security_stability_recovery(&sim, &data);
        assert!(
            recover_sound > recover_wrecked && recover_wrecked >= 0.0,
            "a functioning corps keeps the institutions in better order than a decayed one"
        );

        // A well-ordered ship at the ceiling gets no manufactured order.
        sim.population.stability = cfg.security_stability_recovery_ceiling;
        sim.subsystems.get_mut("security").unwrap().condition = 1.0;
        assert_eq!(
            security_stability_recovery(&sim, &data),
            0.0,
            "a well-governed ship needs no shoring up"
        );
    }

    #[test]
    fn a_failing_engineering_bay_rots_the_whole_ship_faster() {
        // Content-depth subsystems round 7: the engineering bay is the keystone —
        // its condition scales every *other* module's decay. A year with a sound
        // bay wears the medical bay less than a year with a failing one.
        assert!(
            data_swing() > 0.0,
            "this test needs the keystone coupling enabled"
        );

        let wear_med = |eng: f32| -> f32 {
            let (data, mut sim) = campaign(5);
            sim.subsystems.get_mut("engineering_bay").unwrap().condition = eng;
            sim.subsystems.get_mut("medical_bay").unwrap().condition = 0.8;
            decay_subsystems(&mut sim, &data, 1.0);
            0.8 - sim.subsystems["medical_bay"].condition
        };

        let sound = wear_med(1.0); // top-repair bay slows the rot
        let failing = wear_med(0.0); // a failing bay speeds it
        assert!(
            failing > sound,
            "a failing engineering bay should rot the ship faster than a sound one \
             (failing {failing} vs sound {sound})"
        );
    }

    fn data_swing() -> f32 {
        GameData::load()
            .unwrap()
            .config
            .subsystems
            .engineering_decay_swing
    }

    #[test]
    fn a_failing_life_support_plant_thins_the_crew() {
        // Content-depth subsystems round 15: the life-support plant's most
        // fundamental effect. A plant above the failure threshold sustains everyone;
        // one that has collapsed thins the crew each year, worse the further it has
        // failed.
        let data = GameData::load().unwrap();
        let cfg = &data.config.subsystems;
        assert!(
            cfg.life_support_failure_threshold > 0.0 && cfg.life_support_failure_mortality > 0.0,
            "this test needs the life-support mortality coupling enabled"
        );

        let loss_at = |condition: f32| -> u32 {
            let (_, mut sim) = campaign(11);
            sim.population.count = 1000;
            sim.subsystems
                .get_mut("life_support_habitat")
                .unwrap()
                .condition = condition;
            // Isolate the plant: a dead garden contributes no bio life-support
            // (round 17), so this measures the mechanical plant alone.
            sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
            life_support_mortality_loss(&sim, &data)
        };

        // A plant holding above the threshold costs nothing.
        assert_eq!(
            loss_at(cfg.life_support_failure_threshold + 0.1),
            0,
            "a sustaining plant loses no one"
        );
        // A collapsing plant thins the crew, and a worse collapse thins it more.
        let half_failed = loss_at(cfg.life_support_failure_threshold / 2.0);
        let fully_failed = loss_at(0.0);
        assert!(half_failed > 0, "a failing plant costs lives");
        assert!(
            fully_failed > half_failed,
            "a fully collapsed plant thins the crew faster than a half-failed one \
             ({fully_failed} vs {half_failed})"
        );
    }

    #[test]
    fn a_green_garden_helps_the_air_plant_sustain_the_crew() {
        // Content-depth subsystems round 17: the green decks are the ship's lungs. A
        // living agriculture biosphere supplements the failing plant's effective
        // condition, so the same collapsed plant kills far fewer with a thriving
        // garden than with a dead one — real redundancy, but (capped below the
        // threshold) never a wholesale replacement for the plant.
        let data = GameData::load().unwrap();
        let cfg = &data.config.subsystems;
        assert!(
            cfg.agriculture_life_support_contribution > 0.0,
            "this test needs the bio life-support coupling enabled"
        );
        // Capped below the threshold: even a pristine garden cannot alone sustain air.
        assert!(
            cfg.agriculture_life_support_contribution < cfg.life_support_failure_threshold,
            "the garden softens a dead plant, it does not replace it"
        );

        let loss_with_garden = |garden: f32| -> u32 {
            let (_, mut sim) = campaign(17);
            sim.population.count = 1000;
            // A badly collapsed plant, on full power — only the garden differs.
            sim.subsystems
                .get_mut("life_support_habitat")
                .unwrap()
                .condition = 0.0;
            sim.subsystems.get_mut("agriculture").unwrap().condition = garden;
            life_support_mortality_loss(&sim, &data)
        };

        let dead_garden = loss_with_garden(0.0);
        let green_garden = loss_with_garden(1.0);
        assert!(
            dead_garden > 0,
            "a dead plant with no garden thins the crew"
        );
        assert!(
            green_garden < dead_garden,
            "a thriving garden helps the plant sustain more of the crew \
             (green {green_garden} vs dead-garden {dead_garden})"
        );
        assert!(
            green_garden > 0,
            "but a garden alone cannot wholly replace a dead plant"
        );
    }

    #[test]
    fn a_power_starved_plant_kills_even_when_well_repaired() {
        // Content-depth provisioning round 15: a life-support plant needs power as
        // well as repair. A sound plant on a full grid sustains everyone; the same
        // sound plant on a near-empty grid thins the crew — power starvation is as
        // deadly as physical collapse.
        let data = GameData::load().unwrap();
        let critical = data.config.subsystems.life_support_energy_critical;
        assert!(
            critical > 0,
            "this test needs the power-starvation coupling"
        );

        let loss_at_energy = |energy: i64| -> u32 {
            let (_, mut sim) = campaign(13);
            sim.population.count = 1000;
            // A pristine plant — only the grid differs.
            sim.subsystems
                .get_mut("life_support_habitat")
                .unwrap()
                .condition = 1.0;
            // Isolate power: a dead garden contributes no bio life-support (round 17),
            // so only the grid moves the effective condition here.
            sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
            sim.resources.energy = energy;
            life_support_mortality_loss(&sim, &data)
        };

        // A well-powered, sound plant loses no one.
        assert_eq!(
            loss_at_energy(critical * 2),
            0,
            "a plant with power and repair sustains the ship"
        );
        // The same sound plant on a near-dead grid cannot run, and the ship thins.
        assert!(
            loss_at_energy(0) > 0,
            "a sound plant with no current to run it still kills"
        );
    }

    #[test]
    fn a_rotting_farm_feeds_fewer_than_a_pristine_one() {
        // Content-depth subsystems round 12: the food module's condition→output
        // coupling. A pristine farm yields the untouched baseline (factor 1.0),
        // and a degraded one yields proportionally less, so upkeep on the
        // hydroponics pays back every year — not only at the breakdown cliff.
        let (data, mut sim) = campaign(9);
        assert!(
            data.config.subsystems.agriculture_condition_food_penalty > 0.0,
            "this test needs the agriculture condition coupling enabled"
        );

        sim.subsystems.get_mut("agriculture").unwrap().condition = 1.0;
        let pristine = agriculture_condition_food_factor(&sim, &data);
        assert_eq!(pristine, 1.0, "a farm in full repair yields the baseline");

        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.4;
        let neglected = agriculture_condition_food_factor(&sim, &data);
        assert!(
            neglected < pristine,
            "a rotting farm feeds fewer than a pristine one \
             (neglected {neglected} vs pristine {pristine})"
        );
        // The factor never turns food production negative, even at total collapse.
        sim.subsystems.get_mut("agriculture").unwrap().condition = 0.0;
        assert!((0.0..=1.0).contains(&agriculture_condition_food_factor(&sim, &data)));
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
}
