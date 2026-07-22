//! The economic year (GDD §5.1), applied whole on each year boundary of the
//! month clock (W3): production, food upkeep, ship wear, voyage drift,
//! generation/succession, and market drift — the W1-tuned yearly math, split
//! out of `tick.rs` to keep the advance loop readable and the file under the
//! size limit.

use crate::data::{FlavorConfig, GameConfig, GameData, PopulationDelta, ResourceDelta};
use crate::simulation::{crew, legacy, market, ship, subsystems, succession};
use crate::state::sim::SimState;

use super::TickReport;

/// One full economic year (GDD §5.1), applied on a year boundary: production,
/// food upkeep, wear, drift, generation/succession, contract progress, market.
/// Exactly the W1-tuned yearly math — only the clock advance and the (now
/// monthly) event roll live outside it (W3).
pub(super) fn year_boundary_tick(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    let config = &data.config;

    // Production (GDD §5.1: floor(rate * years), one year per tick),
    // multiplied by the serving crew's skills (PLAN item 2). The agriculture
    // subsystem lifts food yield per tier (W5).
    let crew_mult = crew::production_multipliers(sim, data);
    let agri_bonus = subsystems::agriculture_food_bonus(sim, data);
    let produced = ResourceDelta {
        credits: (sim.production.credits * crew_mult.credits).floor() as i64,
        energy: (sim.production.energy * crew_mult.energy).floor() as i64,
        minerals: (sim.production.minerals * crew_mult.minerals).floor() as i64,
        food: (sim.production.food * crew_mult.food * (1.0 + agri_bonus)).floor() as i64,
        influence: (sim.production.influence * crew_mult.influence).floor() as i64,
    };
    sim.resources.apply(&produced);

    // Ship loadout bonus: installed component stats grant extra production and
    // fuel regen (PLAN item 3).
    ship::apply_loadout_effects(sim, data);

    // Food upkeep; famine bleeds morale and people. A serving medic keeps
    // some of the starving alive.
    let upkeep = (sim.population.count as f32 * config.food_per_person_per_year).ceil() as i64;
    if sim.resources.food >= upkeep {
        sim.resources.food -= upkeep;
    } else {
        sim.resources.food = 0;
        let mitigation = 1.0 - crew::famine_loss_reduction(sim, data);
        let losses = (sim.population.count as f32 * 0.02 * mitigation).ceil() as u32;
        sim.population.count = sim.population.count.saturating_sub(losses);
        sim.population.morale = (sim.population.morale - 0.05).max(0.0);
        // A multi-year famine reprinted one line every year (content-depth voice
        // round 6); draw from a pool indexed by year, with the built-in as a
        // fallback so the log never blanks.
        let pool = &config.flavor.famine;
        let line = if pool.is_empty() {
            format!("Rations ran out. The population diminished by {losses}.")
        } else {
            pool[sim.year() as usize % pool.len()].replace("{losses}", &losses.to_string())
        };
        sim.push_log(line);
    }

    // A skilled security chief slowly steadies a fractious ship.
    let recovery = crew::unity_recovery(sim, data);
    if recovery > 0.0 {
        sim.population.unity = (sim.population.unity + recovery).min(1.0);
    }

    // Ship wear, eased while spare parts remain for upkeep (PLAN M4.2). Once
    // the stores run dry the ship wears at full rate — the "held together on
    // hope and prayers" end of a long, unresupplied voyage. Field repair
    // (M4.3) will be the sink that keeps the stores topped up.
    let maintained = sim.ship.spare_parts >= config.parts_upkeep_per_year;
    if maintained {
        sim.ship.spare_parts -= config.parts_upkeep_per_year;
    }
    let wear = if maintained {
        1.0 - config.maintenance_decay_relief
    } else {
        1.0
    };
    // A year spent coasting on empty tanks strains the ship harder — systems
    // shut down and wear runs at the no-fuel multiplier (W4).
    let fuel_factor = if sim.fuel_stalled_this_year {
        config.provisioning.no_fuel_decay_multiplier
    } else {
        1.0
    };
    // A stronger life-support/habitat subsystem slows the life-support wear (W5).
    let ls_reduction = subsystems::life_support_decay_reduction(sim, data);
    sim.ship.hull_integrity =
        (sim.ship.hull_integrity - config.hull_decay_per_year * wear * fuel_factor).max(0.0);
    sim.ship.life_support = (sim.ship.life_support
        - config.life_support_decay_per_year * wear * fuel_factor * (1.0 - ls_reduction))
        .max(0.0);
    if sim.fuel_stalled_this_year {
        // Like famine, a fuel stall reprinted one line per stalled year (voice
        // round 6); pool indexed by year, built-in fallback.
        let pool = &config.flavor.fuel_stall;
        let line = if pool.is_empty() {
            "The tanks ran dry in transit — the ship coasted, and its systems strained in the cold."
                .to_owned()
        } else {
            pool[sim.year() as usize % pool.len()].clone()
        };
        sim.push_log(line);
    }
    sim.fuel_stalled_this_year = false;

    // The rest of the ship's subsystems wear with the years too (W5).
    subsystems::decay_subsystems(sim, data, wear);

    // …and the people whose craft is a module notice when it is left to rot
    // (content-depth subsystems round 8): sustained neglect of a faction's
    // tended subsystem erodes its approval, feeding the round-8 withdrawal.
    sim.apply_subsystem_neglect_sentiment(data);

    // …and give the approval meter a voice (content-depth voice round 8): a people
    // crossing into restlessness or contentment says so in the log, once, so the
    // player feels the mood turn well before a withdrawal beat fires.
    sim.announce_faction_moods(data);

    // Voyage drift (PLAN M4.1): a long voyage changes the people, not just the
    // ship — adaptation and cultural drift rise, loyalty to the founders fades,
    // and the strain wears at morale and unity. Deterministic; the founders'
    // hopeful crew slowly becomes someone else the longer they fly.
    apply_voyage_drift(sim, config);

    // Generational tick (GDD §5.3).
    sim.dynasty.years_since_generation += 1;
    if sim.dynasty.years_since_generation >= config.generation_interval_years {
        let base_index = sim.dynasty.generation as usize;
        for (i, name) in crew::process_generation(sim, data).into_iter().enumerate() {
            // Data-driven so several retirements a generation don't reprint one
            // line (content-depth voice round 5); index by holder so they vary.
            let line =
                FlavorConfig::line_with_name(&data.config.flavor.retirement, base_index + i, &name)
                    .unwrap_or_else(|| format!("{name} stood down from their post."));
            sim.push_log(line);
        }
        let generation = succession::process_generation(sim, data);
        let gen_index = sim.dynasty.generation as usize;
        let flavor = &data.config.flavor;
        for (i, name) in generation.deaths.iter().enumerate() {
            if let Some(line) = FlavorConfig::line_with_name(&flavor.obituary, gen_index + i, name)
            {
                sim.push_log(line);
            }
        }
        if let Some(name) = &generation.new_leader {
            if let Some(line) = FlavorConfig::line_with_name(&flavor.succession, gen_index, name) {
                sim.push_log(line);
            }
        }
        if generation.births > 0 {
            let pool = &flavor.coming_of_age;
            if !pool.is_empty() {
                let line = pool[gen_index % pool.len()]
                    .replace("{generation}", &sim.dynasty.generation.to_string())
                    .replace("{births}", &generation.births.to_string());
                sim.push_log(line);
            }
        }
        if generation.extinct {
            let line = FlavorConfig::line_with_name(&flavor.extinction, gen_index, "")
                .unwrap_or_else(|| "The dynasty has no heirs. The line ends here.".to_owned());
            sim.push_log(line);
            report.dynasty_extinct = true;
        }

        // A generation of drift can quietly fold a dwindling faction into a
        // larger one (W7 soft assimilation).
        if !generation.extinct {
            sim.assimilate_drifted_factions(data);
            // Knowledge dies with the people; the education subsystem passes it
            // forward (W5). A generation with no schooling loses expertise.
            subsystems::transmit_knowledge(sim, data);
        }

        // Each new generation may confront its legacy's defining dilemma
        // (GDD §5.5). Dilemmas always block — they are never delegated.
        if !generation.extinct {
            if let Some(pending) = legacy::roll_dilemma(sim, data) {
                if let Some(dilemma) = data
                    .legacies
                    .get(&sim.legacy.legacy_id)
                    .and_then(|l| l.dilemmas.iter().find(|d| d.id == pending.dilemma_id))
                {
                    sim.push_log(format!(
                        "The new generation faces a reckoning: {}",
                        dilemma.title
                    ));
                }
                sim.pending_dilemma = Some(pending);
                report.decision_required = true;
            }
        }
    }

    // Content-depth voice (round 2): during a long event-less stretch, surface an
    // atmospheric "life aboard" line so the passing centuries read as lived-in.
    // Deterministic — fires once per `ambient_gap_years` of quiet, indexed by
    // year, no RNG, and never resets the event ramp.
    let fl = &config.flavor;
    if fl.ambient_gap_years > 0 && !fl.ambient.is_empty() {
        let years_since = sim.month_clock.saturating_sub(sim.last_event_month_clock) / 12;
        if years_since > 0 && years_since.is_multiple_of(fl.ambient_gap_years) {
            let idx = (sim.year() / fl.ambient_gap_years) as usize % fl.ambient.len();
            sim.push_log(fl.ambient[idx].clone());
        }
    }

    // Market drift closes the economic year. Contract progress is monthly (W2)
    // and the event roll is monthly (W3) — both live in `advance` now; log
    // trimming happens once there too.
    market::drift_prices(sim);
}

/// Apply one year of voyage drift to the population (PLAN M4.1). Identity terms
/// scale by the legacy's multiplier (Adaptors fastest, Preservers slowest); the
/// morale/unity strain is universal. Clamped to 0-1 by `PopulationState::apply`.
pub(super) fn apply_voyage_drift(sim: &mut SimState, config: &GameConfig) {
    let vd = &config.voyage_drift;
    let mult = vd
        .legacy_multipliers
        .get(&sim.legacy.legacy_id)
        .copied()
        .unwrap_or(1.0);
    sim.population.apply(&PopulationDelta {
        adaptation: vd.adaptation_per_year * mult,
        cultural_drift: vd.cultural_drift_per_year * mult,
        legacy_loyalty: vd.legacy_loyalty_per_year * mult,
        morale: vd.morale_strain_per_year,
        unity: vd.unity_strain_per_year,
        ..Default::default()
    });
}
