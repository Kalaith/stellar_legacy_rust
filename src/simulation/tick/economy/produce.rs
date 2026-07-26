//! The year's first fact: what the route costs, what the ship produces,
//! what the people eat, and what the fabricators make of the surplus.

use crate::data::{GameData, ResourceDelta};
use crate::simulation::{crew, ship, subsystems};
use crate::state::sim::SimState;

use super::super::TickReport;
use super::factors::energy_production_factor;
use super::factors::influence_governance_factor;

/// Production, upkeep, the scarcity and plenty streaks, fabrication and
/// the slow spoilage of stores kept past what the holds can cycle.
pub(super) fn produce_and_feed(sim: &mut SimState, data: &GameData, _report: &mut TickReport) {
    let config = &data.config;

    // Route toll (content-depth charters round 13): a charter whose *nature* wears
    // at a ship exacts a steady per-year drain for the whole voyage — hazard's
    // deterministic companion. Read from the template (the contract carries its id),
    // applied before production so the route's standing cost is the year's first fact.
    if let Some(toll) = sim
        .contract
        .as_ref()
        .and_then(|c| data.contracts.get(&c.template_id))
        .map(|t| t.annual_toll.clone())
        .filter(|t| !t.is_none())
    {
        sim.resources.apply(&toll.resource);
        sim.ship.apply(&toll.ship);
        sim.population.apply(&toll.population);
    }

    // Production (GDD §5.1: floor(rate * years), one year per tick),
    // multiplied by the serving crew's skills (PLAN item 2). The agriculture
    // subsystem lifts food yield per tier (W5).
    let crew_mult = crew::production_multipliers(sim, data);
    let agri_bonus = subsystems::agriculture_food_bonus(sim, data);
    // A degraded farm feeds fewer (content-depth subsystems round 12): the food
    // module's condition→output coupling, so upkeep on the hydroponics pays back.
    let agri_condition = subsystems::agriculture_condition_food_factor(sim, data);
    // A council that cannot govern cannot mint the authority its officers spend
    // (content-depth provisioning round 26): influence income falls as governance slips
    // below the line, so a ship in institutional decline earns less of the very political
    // capital its recovery choices cost — the governance twin of the it26 fabrication trap.
    let gov_factor = influence_governance_factor(sim, config);
    // Power runs the factories and refineries (content-depth provisioning round 29): a reactor
    // short of reserve cannot keep the industry at full output, so the ship's *industrial* yield
    // (credits + minerals) is shed while energy sits below its line — food (the grow-lamps, spared
    // first) and energy itself are untouched, so a power crisis cannot cascade into famine.
    let power_factor = energy_production_factor(sim, config);
    let produced = ResourceDelta {
        credits: (sim.production.credits * crew_mult.credits * power_factor).floor() as i64,
        energy: (sim.production.energy * crew_mult.energy).floor() as i64,
        minerals: (sim.production.minerals * crew_mult.minerals * power_factor).floor() as i64,
        food: (sim.production.food * crew_mult.food * (1.0 + agri_bonus) * agri_condition).floor()
            as i64,
        influence: (sim.production.influence * crew_mult.influence * gov_factor).floor() as i64,
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
        // The serving medic *and* the medical bay itself keep some of the
        // starving alive (content-depth subsystems round 9); the combined
        // reduction is capped so a famine is never entirely painless.
        let reduction = (crew::famine_loss_reduction(sim, data)
            + subsystems::medical_famine_relief(sim, data))
        .min(0.9);
        let mitigation = 1.0 - reduction;
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

    // Track how long scarcity has ground on (content-depth provisioning round 13):
    // now that the year's food is settled, a store still below the lean line adds a
    // year to the streak; a recovered larder resets it. This is what lets content
    // tell a chronic hunger from one bad winter.
    if config.lean_food_threshold > 0 && sim.resources.food < config.lean_food_threshold {
        sim.lean_food_years = sim.lean_food_years.saturating_add(1);
    } else {
        sim.lean_food_years = 0;
    }
    // …and its mirror (content-depth provisioning round 14): a store still above the
    // fat line adds a year to the plenty streak, so content can tell a lifetime of
    // abundance — a generation raised never knowing want — from one bumper year.
    if config.fat_food_threshold > 0 && sim.resources.food >= config.fat_food_threshold {
        sim.fat_food_years = sim.fat_food_years.saturating_add(1);
    } else {
        sim.fat_food_years = 0;
    }
    // …and how long the grid has run dark (content-depth provisioning round 34): a store still
    // below the low-energy line adds a year to the power-poverty streak; a recovered grid resets it.
    // This is what lets content tell a chronic power poverty — years of rationed light — from one
    // lean season, the energy twin of the lean-food streak.
    if config.low_energy_threshold > 0 && sim.resources.energy < config.low_energy_threshold {
        sim.lean_energy_years = sim.lean_energy_years.saturating_add(1);
    } else {
        sim.lean_energy_years = 0;
    }

    // Idle reactor output runs the fabricators (content-depth provisioning round 21):
    // energy has no upkeep and simply piles up unused, the voyage's one wasted
    // resource. While the ship holds a real energy surplus *and* the raw minerals to
    // feed them, the fabricators turn spare watts and ore into spare parts — the ship
    // making its own maintenance stock in flight, off power it would otherwise waste.
    // Self-throttling: the run spends energy back toward the line, so it paces itself.
    // The fabrication hall does the fabricating (content-depth subsystems round 26): the
    // engineering bay's condition scales the run's yield, so a neglected hall turns spare
    // power and ore into fewer parts than a sharp one — the coupling that makes the
    // fabricators part of the engineering bay they physically are, not a free background
    // process. Floored at one part when a run happens at all (even improvised hands make
    // something), so a degraded bay slows but never wholly stops the flow.
    let fab_factor = subsystems::engineering_fabrication_factor(sim, data);
    let fab_yield = ((config.fabrication_parts_yield as f32 * fab_factor).round() as i64).max(1);
    if config.surplus_energy_threshold > 0
        && sim.resources.energy >= config.surplus_energy_threshold
        && sim.resources.minerals >= config.fabrication_minerals_cost
        && config.fabrication_parts_yield > 0
    {
        sim.resources.energy -= config.fabrication_energy_cost;
        sim.resources.minerals -= config.fabrication_minerals_cost;
        sim.ship.spare_parts += fab_yield;
        let pool = &data.config.flavor.fabrication;
        let line = if pool.is_empty() {
            format!(
                "The reactors ran easy this year; the fabricators worked spare power and raw ore into {fab_yield} spare parts."
            )
        } else {
            pool[sim.year() as usize % pool.len()].replace("{parts}", &fab_yield.to_string())
        };
        sim.push_log(line);
    }

    // Stores kept past what the ship can keep *fresh* spoil (content-depth provisioning
    // round 24): food is the one resource with no upkeep and no cap, so it could pile up
    // without limit — but a generation ship's cold-holds and hydroponics can only cycle so
    // much, and everything beyond that carrying capacity slowly rots. A gentle soft cap:
    // each year a fraction of the *excess above capacity* is lost, so a ship at sensible
    // stores loses nothing and only a deep hoard erodes, asymptoting toward the line it can
    // actually keep. Bounds the abundance without forbidding a prudent reserve.
    if config.food_carrying_capacity > 0 && sim.resources.food > config.food_carrying_capacity {
        let excess = sim.resources.food - config.food_carrying_capacity;
        let spoiled = (excess as f32 * config.food_spoilage_fraction).round() as i64;
        if spoiled > 0 {
            sim.resources.food -= spoiled;
            let pool = &data.config.flavor.food_spoilage;
            if !pool.is_empty() {
                let line = pool[sim.year() as usize % pool.len()]
                    .replace("{spoiled}", &spoiled.to_string());
                sim.push_log(line);
            }
        }
    }
}
