//! What the year did to the hull, the air, the modules, and the will to
//! keep them up.

use crate::data::GameData;
use crate::simulation::subsystems;
use crate::state::sim::SimState;

use super::super::TickReport;

/// Wear eased by spare parts, the disrepair and becalmed streaks it feeds,
/// and the heart a ship going nowhere slowly loses.
pub(super) fn wear_the_ship(sim: &mut SimState, data: &GameData, _report: &mut TickReport) {
    let config = &data.config;

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
    // Track how long the ship has gone unmended (content-depth provisioning round 27): a year
    // short of the upkeep stock extends the disrepair, a year that can cover it clears the
    // count. This is what lets a bad year between resupplies be told from a chronic disrepair
    // — a ship held together with tape for a generation — and drives the disrepair morale
    // drain below.
    if maintained {
        sim.lean_parts_years = 0;
    } else {
        sim.lean_parts_years = sim.lean_parts_years.saturating_add(1);
    }
    // A ship where nothing gets fixed wears the crew's heart (content-depth provisioning round
    // 27): a chronic disrepair grinds at morale the way a chronic hunger (it89/round 17) and a
    // chronic becalming (round 25) do — the third of the sustained-privation morale costs,
    // completing the trio of larder, drive, and toolroom. Threshold-gated on the same
    // "years-to-chronic" line, so one lean year between resupplies is inert; only a sustained
    // disrepair — deck plates left buckled, seals left weeping, the crew watching their home
    // slowly come apart with nothing to mend it — bites.
    if config.disrepair_morale_drain > 0.0
        && config.chronic_hunger_years > 0
        && sim.lean_parts_years >= config.chronic_hunger_years
    {
        sim.population.morale = (sim.population.morale - config.disrepair_morale_drain).max(0.0);
    }
    // A year spent coasting on empty tanks strains the ship harder — systems
    // shut down and wear runs at the no-fuel multiplier (W4).
    let fuel_factor = if sim.fuel_stalled_this_year {
        config.provisioning.no_fuel_decay_multiplier
    } else {
        1.0
    };
    // A stronger life-support/habitat subsystem slows the life-support wear (W5).
    let ls_reduction = subsystems::life_support_decay_reduction(sim, data);
    // …and the engineering bay maintains the *hull* too (content-depth subsystems round
    // 24): the ship is mended where the ship is mended, so a rotting bay lets the frame
    // wear faster while a sound one holds it at the baseline rate — extending the it62
    // decay keystone from the modules to the ship's own structure, and compounding the
    // it hull-collapse spiral (a failed bay hastens the hull toward its red line).
    let hull_decay_factor = subsystems::engineering_hull_decay_factor(sim, data);
    sim.ship.hull_integrity = (sim.ship.hull_integrity
        - config.hull_decay_per_year * wear * fuel_factor * hull_decay_factor)
        .max(0.0);
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
    // Track how long the ship has been becalmed (content-depth campaign-skeleton round
    // 25): a stalled year extends the stranding; a year that burns clears it. This is
    // what lets a bad month coasting be told from a genuine stranding, and drives the
    // becalmed beat.
    if sim.fuel_stalled_this_year {
        sim.fuel_stall_years = sim.fuel_stall_years.saturating_add(1);
    } else {
        sim.fuel_stall_years = 0;
    }
    sim.fuel_stalled_this_year = false;
    // A ship going nowhere loses heart (content-depth provisioning round 25): a chronic
    // becalming wears the crew's spirits the way a chronic hunger does (it89/round 17),
    // the standing cost beside the it25 becalmed *beat* — the beat reckons with the
    // stranding once, this grinds at morale every year it holds. Threshold-gated so a bad
    // month coasting is inert; only a sustained stranding bites.
    if config.becalmed_morale_drain > 0.0
        && config.chronic_hunger_years > 0
        && sim.fuel_stall_years >= config.chronic_hunger_years
    {
        sim.population.morale = (sim.population.morale - config.becalmed_morale_drain).max(0.0);
    }
    // …and a ship long run in the dark loses heart too (content-depth provisioning round 34): the
    // third great privation drain, beside the it17 chronic hunger and the it25 becalming. A grid
    // held below the low line for years — rationed light, cold decks, systems cycled off to keep the
    // essential ones lit — wears the crew's spirit every year it holds, the standing morale cost the
    // it33 power *voice* only narrates. Same sustained gate as the hunger and becalming drains, so a
    // single lean season is inert; only a chronic power poverty bites.
    if config.chronic_low_energy_morale_drain > 0.0
        && config.chronic_hunger_years > 0
        && sim.lean_energy_years >= config.chronic_hunger_years
    {
        sim.population.morale =
            (sim.population.morale - config.chronic_low_energy_morale_drain).max(0.0);
    }

    // The rest of the ship's subsystems wear with the years too (W5).
    subsystems::decay_subsystems(sim, data, wear);
}
