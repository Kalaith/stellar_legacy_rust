//! The economic year (GDD §5.1), applied whole on each year boundary of the
//! month clock (W3): production, food upkeep, ship wear, voyage drift,
//! generation/succession, and market drift — the W1-tuned yearly math, split
//! out of `tick.rs` to keep the advance loop readable and the file under the
//! size limit.

use crate::data::{GameConfig, GameData, PopulationDelta, ResourceDelta};
use crate::simulation::{crew, legacy, market, mortality, ship, subsystems, succession};
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
    // A council that cannot govern cannot mint the authority its officers spend
    // (content-depth provisioning round 26): influence income falls as governance slips
    // below the line, so a ship in institutional decline earns less of the very political
    // capital its recovery choices cost — the governance twin of the it26 fabrication trap.
    let gov_factor = influence_governance_factor(sim, config);
    let produced = ResourceDelta {
        credits: (sim.production.credits * crew_mult.credits).floor() as i64,
        energy: (sim.production.energy * crew_mult.energy).floor() as i64,
        minerals: (sim.production.minerals * crew_mult.minerals).floor() as i64,
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

    // A life-support plant that has failed badly cannot sustain everyone (content-depth
    // subsystems round 15): the module's most fundamental effect. Below the failure
    // threshold the ship thins each year, scaled by how far the plant has collapsed.
    let ls_loss = subsystems::life_support_mortality_loss(sim, data);
    if ls_loss > 0 {
        sim.population.count = sim.population.count.saturating_sub(ls_loss);
        // Pooled so a failing-air stretch doesn't reprint one line every year it holds
        // (content-depth voice round 24); indexed by year, built-in fallback.
        let pool = &data.config.flavor.life_support_loss;
        let line = if pool.is_empty() {
            format!(
                "The failing life-support could not hold the whole ship in breathable air; {ls_loss} were lost to the thinning decks."
            )
        } else {
            pool[sim.year() as usize % pool.len()].replace("{losses}", &ls_loss.to_string())
        };
        sim.push_log(line);
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
    // …and whether the aboard peoples *get along* touches cohesion too (content-depth
    // factions round 23): where the coupling above reads how *content* they are, this
    // reads how they stand *to each other* — two large aboard rivals sharing a hull
    // grind at unity year over year (a standing friction, not only the it14 event-time
    // spillover), while an aboard allied bloc lifts it. So the *composition* of the
    // roster, not just its mood, is a standing cohesion cost or dividend.
    sim.apply_faction_relationship_cohesion(data);

    // A divided ship is harder to govern (content-depth factions round 18): where the
    // approval→unity coupling reads how *content* the peoples are, this reads how
    // ideologically *split* they are — a coalition spanning the tech↔tradition spectrum
    // strains the institutions, eroding `stability` each year its spread runs past the
    // threshold. Distinct from cohesion: a polity can be content yet fractious. A
    // single-minded ship (spread below the line) governs freely; a well-kept security
    // corps (it108) can offset the drain. Neutral only within the tolerated spread.
    let spread_penalty = data.config.factions.ideology_spread_stability_penalty;
    if spread_penalty > 0.0 {
        let excess = (sim.aboard_ideology_spread(data)
            - data.config.factions.ideology_spread_threshold)
            .max(0.0);
        if excess > 0.0 {
            sim.population.stability =
                (sim.population.stability - spread_penalty * excess).max(0.0);
        }
    }

    // The habitat is where the people live (content-depth subsystems round 11): a
    // home kept sound lifts the ship's morale year over year, a failing one drags
    // it — the one maintenance-driven counterweight morale has to the voyage strain.
    let habitat = subsystems::habitat_morale_effect(sim, data);
    if habitat != 0.0 {
        sim.population.morale = (sim.population.morale + habitat).clamp(0.0, 1.0);
    }
    // …and the ship's *cultural* life is the other pillar of its spirits (content-depth
    // subsystems round 22): a living education/culture module — schools, arts, the
    // year's festivals, the shared story — lifts morale the way a sound home does, and a
    // hollowed-out one drags it, so a crew can be warm and fed and still grim. The
    // cultural twin of the habitat morale swing, completing morale's environmental map.
    let culture = subsystems::education_morale_effect(sim, data);
    if culture != 0.0 {
        sim.population.morale = (sim.population.morale + culture).clamp(0.0, 1.0);
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
    // …and the long plenty lifts them (content-depth provisioning round 20): the
    // morale mirror of the chronic-hunger drain, on the same "sustained" threshold —
    // a well-fed generation is a happy one, so a fat spell held past `chronic_hunger_
    // years` adds a little morale each year, completing the provisioning→morale pole
    // (hunger wears the spirit, plenty eases it) beside the death/birth poles.
    if config.sustained_plenty_morale_lift > 0.0
        && config.chronic_hunger_years > 0
        && sim.fat_food_years >= config.chronic_hunger_years
    {
        sim.population.morale =
            (sim.population.morale + config.sustained_plenty_morale_lift).min(1.0);
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

    // The rest of the ship's subsystems wear with the years too (W5).
    subsystems::decay_subsystems(sim, data, wear);

    // …and the people whose craft is a module notice when it is left to rot
    // (content-depth subsystems round 8): sustained neglect of a faction's
    // tended subsystem erodes its approval, feeding the round-8 withdrawal.
    sim.apply_subsystem_neglect_sentiment(data);
    // …and its bright mirror (content-depth factions round 22): a people *delighted*
    // with its lot tends its module with pride, keeping it a shade sharper than duty
    // alone would — so a kept module keeps its people content and content people keep
    // the module kept, a virtuous circle across the faction↔subsystem boundary.
    sim.apply_proud_tender_upkeep(data);

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
    // …and give the crew's *devotion to the founders' mission* a voice (content-depth
    // voice round 20), now that the year's drift has eroded loyalty and any event
    // shifts have settled: when loyalty crosses into a guttering band (the founders'
    // purpose fading to a story) or a bright one (the dream taken up afresh), the decks
    // remark it once — the identity-side twin of the spirits and governance voices, and
    // the voice that gives the game's core theme (a ship forgetting why it flies) a line.
    sim.announce_loyalty_mood(data);
    // …and give the crew's *bodies* a voice too (content-depth voice round 25), now that
    // the year's drift (and the it25 medical resistance to it) has settled on adaptation:
    // when the descendants cross into a shipborn body or hold to the founders' baseline,
    // the decks remark the crew becoming, or refusing to become, a new kind of people —
    // the physiological companion to the loyalty (their belief) voice above.
    sim.announce_adaptation_mood(data);
    // …and give the crew's *cohesion* a voice (content-depth voice round 21), now that
    // the year's faction-mood coupling (it100), security recovery, and voyage strain
    // have all settled on unity: when the crew crosses into fraying into cliques or
    // pulling back together as one, the decks remark it once — the internal-state voice
    // beside the spirits, the governance, and the founders' fire.
    sim.announce_unity_mood(data);
    // …and the ship's *own body* has a voice too (content-depth voice round 22), now that
    // the year's hull wear (and any refit) has settled: when the hull crosses into a
    // groaning band or back into a sound one, the decks remark the vessel itself aging or
    // renewed — the first voice for the machine rather than the crew it carries.
    sim.announce_hull_condition(data);
    // …and the ship's *air* is the other half of its body (content-depth voice round 23),
    // now that the year's life-support wear (and any repair) has settled: when the air
    // crosses into a stale band or back into a fresh one, the decks remark the atmosphere
    // going close or clearing — the structure and the air, the two survival systems, both
    // now speak.
    sim.announce_air_condition(data);

    // Founding Day (real-time loop follow-up): everyone gains a year at once, and
    // any officer aged past their term stands down. Aging is yearly; death is the
    // separate monthly roll in `mortality::monthly_tick` (driven from the tick).
    mortality::annual_aging(sim, data);

    // Generational renewal (GDD §5.3): every interval a new cohort comes of age.
    // Aging, death, and succession are continuous now and live in `mortality`;
    // this tick only adds the young and runs the once-a-generation beats.
    sim.dynasty.years_since_generation += 1;
    if !sim.dynasty.extinct
        && sim.dynasty.years_since_generation >= config.generation_interval_years
    {
        let births = succession::process_generation(sim, data);
        let gen_index = sim.dynasty.generation as usize;
        let flavor = &data.config.flavor;
        if births > 0 && !flavor.coming_of_age.is_empty() {
            let pool = &flavor.coming_of_age;
            let line = pool[gen_index % pool.len()]
                .replace("{generation}", &sim.dynasty.generation.to_string())
                .replace("{births}", &births.to_string());
            sim.push_log(line);
        }

        // Each people's numbers wax or wane over the generations (content-depth
        // factions round 11): the balance of power shifts, so which people runs
        // the ship can change mid-voyage. Applied before assimilation, so a people
        // that dwindles far enough can then be folded into a larger one.
        sim.apply_faction_demographic_drift(data);

        // A generation of drift can quietly fold a dwindling faction into a
        // larger one (W7 soft assimilation).
        sim.assimilate_drifted_factions(data);
        // Knowledge dies with the people; the education subsystem passes it
        // forward (W5). A generation with no schooling loses expertise.
        subsystems::transmit_knowledge(sim, data);

        // Each new generation may confront its legacy's defining dilemma
        // (GDD §5.5). Dilemmas always block — they are never delegated.
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

    // Content-depth voice (round 2): during a long event-less stretch, surface an
    // atmospheric "life aboard" line so the passing centuries read as lived-in.
    // Deterministic — fires once per `ambient_gap_years` of quiet, indexed by
    // year, no RNG, and never resets the event ramp.
    let fl = &config.flavor;
    if fl.ambient_gap_years > 0 {
        let years_since = sim.month_clock.saturating_sub(sim.last_event_month_clock) / 12;
        if years_since > 0 && years_since.is_multiple_of(fl.ambient_gap_years) {
            // The quiet reads differently as the ship changes (see
            // `quiet_ambient_pool`): the grim/flush *conditions* first, and failing
            // all of them, the plain "ordinary" quiet colored by *who runs the ship*.
            let pool = quiet_ambient_pool(sim, data);
            if !pool.is_empty() {
                let idx = (sim.year() / fl.ambient_gap_years) as usize % pool.len();
                sim.push_log(pool[idx].clone());
            }
        }
    }

    // Legible fuel provisioning (real-time loop follow-up: stat changes should
    // read as *something the ship did*). The tank sags monthly with the burn and
    // is topped up yearly by the drive's scoop — a sawtooth that used to move with
    // no word in the log. Periodically report the fuel actually gathered since the
    // last note, so the rise has an in-world cause. Self-throttling: the accrual
    // only grows while the tank has room to take on fuel (i.e. while a crossing is
    // drawing it down), so a ship sitting on a full tank on-station stays silent.
    if fl.fuel_report_gap_years > 0
        && !fl.fuel_gain.is_empty()
        && sim.year() > 0
        && sim.year().is_multiple_of(fl.fuel_report_gap_years)
    {
        let amount = (sim.fuel_scooped_accum * 100.0).round() as i64;
        if amount >= 5 {
            let idx = (sim.year() / fl.fuel_report_gap_years) as usize % fl.fuel_gain.len();
            sim.push_log(fl.fuel_gain[idx].replace("{amount}", &amount.to_string()));
            sim.fuel_scooped_accum = 0.0;
        }
    }

    // Market drift closes the economic year. Contract progress is monthly (W2)
    // and the event roll is monthly (W3) — both live in `advance` now; log
    // trimming happens once there too.
    market::drift_prices(sim);
}

/// The ambient "life aboard" pool a quiet year draws from. The ship's *condition*
/// takes precedence — a long hunger, then a hollowed-out crew, then a far-drifted
/// people, then a long-flush larder (grim notes loudest first) — and failing all
/// of them, the plain *ordinary* quiet is colored by *who runs the ship*: a clear
/// dominant people's own `ambient` lines (content-depth factions round 21), or the
/// generic ordinary pool when it has none. Returns an empty pool only if every
/// candidate is empty.
pub(super) fn quiet_ambient_pool<'a>(sim: &SimState, data: &'a GameData) -> &'a Vec<String> {
    let fl = &data.config.flavor;
    if !fl.ambient_lean.is_empty()
        && fl.ambient_lean_years_threshold > 0
        && sim.lean_food_years >= fl.ambient_lean_years_threshold
    {
        return &fl.ambient_lean;
    }
    if !fl.ambient_hollow.is_empty() && sim.population.count <= fl.ambient_population_threshold {
        return &fl.ambient_hollow;
    }
    if !fl.ambient_drifted.is_empty() && sim.population.cultural_drift >= fl.ambient_drift_threshold
    {
        return &fl.ambient_drifted;
    }
    if !fl.ambient_fat.is_empty()
        && fl.ambient_fat_years_threshold > 0
        && sim.fat_food_years >= fl.ambient_fat_years_threshold
    {
        return &fl.ambient_fat;
    }
    // Ordinary quiet — colored by the largest aboard people if it has ambient lines.
    sim.dominant_faction_id()
        .and_then(|id| data.factions.get(id))
        .map(|f| &f.ambient)
        .filter(|a| !a.is_empty())
        .unwrap_or(&fl.ambient)
}

/// Apply one year of voyage drift to the population (PLAN M4.1). Identity terms
/// scale by the legacy's multiplier (Adaptors fastest, Preservers slowest); the
/// morale/unity strain is universal. Clamped to 0-1 by `PopulationState::apply`.
/// The fraction of its influence income a ship actually mints given its governance
/// (content-depth provisioning round 26). Influence is political capital, and a council
/// that cannot reach quorum cannot issue the authority its officers spend: at or above the
/// governance line the ship earns full income (factor 1.0); below it, the factor falls
/// linearly from 1.0 at the line toward `influence_governance_floor` at zero stability, so
/// even an ungoverned ship mints a little raw standing but never zero. Inert (1.0) when the
/// threshold is 0. Reads `stability` only — deterministic, no RNG.
pub(super) fn influence_governance_factor(sim: &SimState, config: &GameConfig) -> f32 {
    let threshold = config.influence_governance_threshold;
    if threshold <= 0.0 {
        return 1.0;
    }
    let stability = sim.population.stability;
    if stability >= threshold {
        return 1.0;
    }
    let floor = config.influence_governance_floor;
    floor + (1.0 - floor) * (stability / threshold)
}

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
    // A well-kept infirmary keeps the crew *physically* baseline (content-depth
    // subsystems round 25): the bodily twin of the archive's cultural resistance. The
    // medical bay's living craft (its knowledge) slows the shipborn adaptation the way
    // the archive slows the cultural drift, so a ship bound for a world can hold its crew
    // fit to live on one. Reads knowledge, not condition — it is the *craft* of managing
    // the body, like the archive is the *memory* of the founders.
    let medical_knowledge = sim
        .subsystems
        .get("medical_bay")
        .map_or(0.0, |s| s.knowledge);
    let adaptation_mult =
        identity_mult * (1.0 - vd.medical_adaptation_resistance * medical_knowledge).max(0.0);
    sim.population.apply(&PopulationDelta {
        adaptation: vd.adaptation_per_year * adaptation_mult,
        cultural_drift: vd.cultural_drift_per_year * culture_mult,
        legacy_loyalty: vd.legacy_loyalty_per_year * culture_mult,
        morale: vd.morale_strain_per_year,
        unity: vd.unity_strain_per_year,
        ..Default::default()
    });
}
