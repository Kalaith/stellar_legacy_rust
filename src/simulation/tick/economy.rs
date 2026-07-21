//! The economic year (GDD §5.1), applied whole on each year boundary of the
//! month clock (W3): production, food upkeep, ship wear, voyage drift,
//! generation/succession, and market drift — the W1-tuned yearly math, split
//! out of `tick.rs` to keep the advance loop readable and the file under the
//! size limit.

use crate::data::{GameConfig, GameData, PopulationDelta, ResourceDelta};
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
        sim.push_log(format!(
            "Rations ran out. The population diminished by {losses}."
        ));
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
        sim.push_log(
            "The tanks ran dry in transit — the ship coasted, and its systems strained in the cold.",
        );
    }
    sim.fuel_stalled_this_year = false;

    // The rest of the ship's subsystems wear with the years too (W5).
    subsystems::decay_subsystems(sim, data, wear);

    // Voyage drift (PLAN M4.1): a long voyage changes the people, not just the
    // ship — adaptation and cultural drift rise, loyalty to the founders fades,
    // and the strain wears at morale and unity. Deterministic; the founders'
    // hopeful crew slowly becomes someone else the longer they fly.
    apply_voyage_drift(sim, config);

    // Generational tick (GDD §5.3).
    sim.dynasty.years_since_generation += 1;
    if sim.dynasty.years_since_generation >= config.generation_interval_years {
        for name in crew::process_generation(sim, data) {
            sim.push_log(format!("{name} stood down from their post."));
        }
        let generation = succession::process_generation(sim, data);
        for name in &generation.deaths {
            sim.push_log(format!("{name} was laid to rest among the stars."));
        }
        if let Some(name) = &generation.new_leader {
            sim.push_log(format!("{name} assumed leadership of the dynasty."));
        }
        if generation.births > 0 {
            sim.push_log(format!(
                "Generation {} came of age: {} new dynasty member(s).",
                sim.dynasty.generation, generation.births
            ));
        }
        if generation.extinct {
            sim.push_log("The dynasty has no heirs. The line ends here.");
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
