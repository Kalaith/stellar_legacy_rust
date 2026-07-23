//! The economic year (GDD §5.1), applied whole on each year boundary of the
//! month clock (W3): production, food upkeep, ship wear, voyage drift,
//! generation/succession, and market drift — the W1-tuned yearly math, split
//! out of `tick.rs` to keep the advance loop readable and the file under the
//! size limit.

use crate::data::{FlavorConfig, GameData, PopulationDelta, ResourceDelta};
use crate::simulation::{crew, legacy, market, ship, subsystems, succession};
use crate::state::sim::SimState;

use super::TickReport;

/// One full economic year (GDD §5.1), applied on a year boundary: production,
/// food upkeep, wear, drift, generation/succession, contract progress, market.
/// Exactly the W1-tuned yearly math — only the clock advance and the (now
/// monthly) event roll live outside it (W3).
pub(super) fn year_boundary_tick(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
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
    let produced = ResourceDelta {
        credits: (sim.production.credits * crew_mult.credits).floor() as i64,
        energy: (sim.production.energy * crew_mult.energy).floor() as i64,
        minerals: (sim.production.minerals * crew_mult.minerals).floor() as i64,
        food: (sim.production.food * crew_mult.food * (1.0 + agri_bonus) * agri_condition).floor()
            as i64,
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

    // A life-support plant that has failed badly cannot sustain everyone (content-depth
    // subsystems round 15): the module's most fundamental effect. Below the failure
    // threshold the ship thins each year, scaled by how far the plant has collapsed.
    let ls_loss = subsystems::life_support_mortality_loss(sim, data);
    if ls_loss > 0 {
        sim.population.count = sim.population.count.saturating_sub(ls_loss);
        sim.push_log(format!(
            "The failing life-support could not hold the whole ship in breathable air; {ls_loss} were lost to the thinning decks."
        ));
    }

    // A skilled security chief and a well-kept security corps both slowly steady
    // a fractious ship (content-depth subsystems round 9): crew skill + module
    // condition stack.
    let recovery = crew::unity_recovery(sim, data) + subsystems::security_unity_recovery(sim, data);
    if recovery > 0.0 {
        sim.population.unity = (sim.population.unity + recovery).min(1.0);
    }

    // A functioning security/justice corps also keeps the ship's *institutions*
    // in order (content-depth subsystems round 16): stability's first maintenance
    // counterweight, steadying a fracturing government toward the ceiling.
    let stability_recovery = subsystems::security_stability_recovery(sim, data);
    if stability_recovery > 0.0 {
        sim.population.stability = (sim.population.stability + stability_recovery).min(1.0);
    }

    // A ship holds together as well as its peoples are content (content-depth
    // factions round 15): the faction system finally touches the ship's own
    // cohesion. Each year unity drifts by the member-weighted mood of the aboard
    // peoples — a content polity steadies the ship, a resentful one erodes it —
    // so mistreating your factions doesn't only risk their departure, it wears at
    // the unity the crisis and recovery beats turn on. Neutral mood (0.5) is inert.
    let cohesion = data.config.factions.approval_unity_coupling;
    if cohesion != 0.0 {
        let mood = sim.aboard_approval_mean();
        sim.population.unity = (sim.population.unity + cohesion * (mood - 0.5)).clamp(0.0, 1.0);
    }

    // The habitat is where the people live (content-depth subsystems round 11): a
    // home kept sound lifts the ship's morale year over year, a failing one drags
    // it — the one maintenance-driven counterweight morale has to the voyage strain.
    let habitat = subsystems::habitat_morale_effect(sim, data);
    if habitat != 0.0 {
        sim.population.morale = (sim.population.morale + habitat).clamp(0.0, 1.0);
    }

    // The long lean wears the crew down (content-depth provisioning round 17): the
    // provisioning axis's first *systemic* coupling — where every prior scarcity
    // mechanic was an event gate or a counter, a hunger that has ground on for years
    // now bites the year tick directly. The it89 lean-years counter, until now only
    // gating content and the drift-aware ambient (voice r13), gets a mechanical toll:
    // a chronic hunger doesn't merely read hungry, it *is* wearing. Threshold-gated so
    // one bad winter is inert (the acute famine events' domain) — only a sustained
    // lean grinds the crew's spirits down, and via the ship-mood voice the decks
    // audibly go heavy as it does.
    if config.chronic_hunger_morale_drain > 0.0
        && sim.lean_food_years >= config.chronic_hunger_years
    {
        sim.population.morale =
            (sim.population.morale - config.chronic_hunger_morale_drain).max(0.0);
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
    // …and give the ship's whole political climate a voice (content-depth voice
    // round 15): when the aggregate mood of the peoples crosses into broad
    // discontent or broad ease, the polity as a whole says so once — the ship-level
    // companion to the per-faction and morale voices.
    sim.announce_polity_mood(data);
    // …and the standing character of whoever runs the ship bends its reputation over
    // the generations (content-depth factions round 16): a kind majority drifts the
    // ship toward a merciful name, a cold one hardens it, no dramatic choice required.
    sim.apply_dominant_reputation_lean(data);
    // …and the ship remarks when its name begins to mean something (content-depth
    // voice round 16): a merciful or a feared reputation says so once, at a gentler
    // threshold than the it109 beat — the quiet marker before the defining reckoning.
    sim.announce_reputation_name(data);

    // Voyage drift (PLAN M4.1): a long voyage changes the people, not just the
    // ship — adaptation and cultural drift rise, loyalty to the founders fades,
    // and the strain wears at morale and unity. Deterministic; the founders'
    // hopeful crew slowly becomes someone else the longer they fly.
    apply_voyage_drift(sim, data);

    // …and give the ship's *collective* morale a voice (content-depth voice round
    // 11), now that the year's habitat lift and voyage strain have both settled:
    // when the whole crew's spirits cross into a grim or a buoyant band, the decks
    // say so once — the ship-wide twin of the faction-mood announcement above.
    sim.announce_ship_mood(data);
    // …and give the ship's *institutions* a voice (content-depth voice round 17), now
    // that the year's security-driven recovery and any event shifts have settled: when
    // stability crosses into a fraying or a firm band the decks remark the government
    // slipping or working — the governance twin of the spirits and political-climate
    // voices above.
    sim.announce_stability_mood(data);

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

        // Each people's numbers wax or wane over the generations (content-depth
        // factions round 11): the balance of power shifts, so which people runs
        // the ship can change mid-voyage. Applied before assimilation, so a people
        // that dwindles far enough can then be folded into a larger one.
        if !generation.extinct {
            sim.apply_faction_demographic_drift(data);
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
    if fl.ambient_gap_years > 0 {
        let years_since = sim.month_clock.saturating_sub(sim.last_event_month_clock) / 12;
        if years_since > 0 && years_since.is_multiple_of(fl.ambient_gap_years) {
            // The quiet reads differently as the ship changes, in order of how
            // loudly each condition speaks in a silence: a long hunger (round 13)
            // above a hollowed-out crew (round 12) above a far-drifted people
            // (round 10) — the grim notes first, loudest to quietest. Failing all
            // three, a *long-prosperous* ship's quiet reads fat and easy (round 14,
            // the first positive-condition ambient); only a ship neither grim nor
            // notably flush reads the plain ordinary.
            let pool = if !fl.ambient_lean.is_empty()
                && sim.lean_food_years >= fl.ambient_lean_years_threshold
                && fl.ambient_lean_years_threshold > 0
            {
                &fl.ambient_lean
            } else if !fl.ambient_hollow.is_empty()
                && sim.population.count <= fl.ambient_population_threshold
            {
                &fl.ambient_hollow
            } else if !fl.ambient_drifted.is_empty()
                && sim.population.cultural_drift >= fl.ambient_drift_threshold
            {
                &fl.ambient_drifted
            } else if !fl.ambient_fat.is_empty()
                && fl.ambient_fat_years_threshold > 0
                && sim.fat_food_years >= fl.ambient_fat_years_threshold
            {
                &fl.ambient_fat
            } else {
                &fl.ambient
            };
            if !pool.is_empty() {
                let idx = (sim.year() / fl.ambient_gap_years) as usize % pool.len();
                sim.push_log(pool[idx].clone());
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
pub(super) fn apply_voyage_drift(sim: &mut SimState, data: &GameData) {
    let vd = &data.config.voyage_drift;
    let legacy_mult = vd
        .legacy_multipliers
        .get(&sim.legacy.legacy_id)
        .copied()
        .unwrap_or(1.0);
    // Who runs the ship bends how fast the people drift from the founders
    // (content-depth factions round 9): the dominant faction's ideology finally
    // does something — a tech-embracing majority leans into becoming someone new,
    // a tradition-bound one holds the founders' line. Read before the mutable
    // apply; gentle enough that identity still moves the same way whoever leads.
    let ideology = sim
        .dominant_faction_id()
        .and_then(|id| data.factions.get(id))
        .map_or(0.0, |f| f.ideology);
    let identity_mult = legacy_mult * (1.0 + vd.dominant_ideology_scale * ideology).max(0.0);
    // A well-kept culture archive resists the people forgetting the founders
    // (content-depth subsystems round 10): the education/culture module's
    // *knowledge* — how much of the founding is still remembered — slows the
    // cultural drift and the loyalty fade, but not the body's physiological
    // adaptation to the ship, which happens whether or not the archive holds.
    let archive_knowledge = sim
        .subsystems
        .get("education_culture")
        .map_or(0.0, |s| s.knowledge);
    let culture_mult =
        identity_mult * (1.0 - vd.archive_drift_resistance * archive_knowledge).max(0.0);
    sim.population.apply(&PopulationDelta {
        adaptation: vd.adaptation_per_year * identity_mult,
        cultural_drift: vd.cultural_drift_per_year * culture_mult,
        legacy_loyalty: vd.legacy_loyalty_per_year * culture_mult,
        morale: vd.morale_strain_per_year,
        unity: vd.unity_strain_per_year,
        ..Default::default()
    });
}
