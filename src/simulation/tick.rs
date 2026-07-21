//! The simulation tick (GDD §3 step 3, §5.1).
//!
//! Time advances only on explicit player action (Pillar 4). `advance` steps the
//! month clock forward by the current speed step, applying the W1-tuned economic
//! year on each year boundary (production, upkeep, wear, aging, contract
//! progress, market) and rolling for a dated event every month — hard-stopping
//! the instant a decision, completion, or extinction lands (W3).

use crate::data::contracts::ContractPhase;
use crate::data::{GameConfig, GameData, PopulationDelta, ResourceDelta};
use crate::simulation::contract::SuccessLevel;
use crate::simulation::{contract, crew, event_resolver, legacy, market, ship, succession};
use crate::state::sim::{SimState, SpeedStep};

/// Everything a single year produced that the caller (game.rs) must react
/// to: log lines are already recorded on the sim; a completed contract and a
/// blocking event are surfaced explicitly.
#[derive(Debug, Default)]
pub struct TickReport {
    /// Set when the active contract reached its target duration this year.
    pub contract_completed: Option<(f32, SuccessLevel)>,
    /// Set when an event fired that needs a council decision (not delegated).
    pub decision_required: bool,
    pub dynasty_extinct: bool,
    /// Months actually advanced by this call before it stopped (W3). Less than
    /// the speed step's span whenever a decision hard-stops the advance early.
    pub months_advanced: u32,
    /// Set when the active contract crossed into a new authored phase this call
    /// (W2) — a hard-stop for the fast-forward, like a decision.
    pub phase_changed: Option<ContractPhase>,
}

/// Advance time by the current speed step (W3). Steps month by month, applying
/// the W1-tuned economic year on each year boundary and rolling for events every
/// month, and hard-stops the instant a council decision, contract completion, or
/// extinction lands — so a single Advance press never skips past a moment that
/// needs the player (Pillar 4).
pub fn advance(sim: &mut SimState, data: &GameData) -> TickReport {
    debug_assert!(
        !sim.has_pending_decision(),
        "caller must resolve the pending event/dilemma before advancing time"
    );
    let mut report = TickReport::default();

    for _ in 0..sim.speed.months() {
        sim.month_clock += 1;
        report.months_advanced += 1;

        // The economic tick applies whole, on the year boundary — the W1 math
        // is untouched; only its cadence is now driven by the month clock.
        if sim.month_clock.is_multiple_of(12) {
            year_boundary_tick(sim, data, &mut report);
        }

        // Monthly contract progress (W2): objective accrual on-station, the
        // authored phase timeline, milestones, and completion all step here.
        month_of_contract(sim, data, &mut report);

        // Monthly event roll (GDD §5.4), dated to this exact month. Skipped on a
        // month that already produced a blocking dilemma, a completion, or an
        // extinction — one decision at a time, never piled onto a finished year.
        if sim.pending_dilemma.is_none()
            && report.contract_completed.is_none()
            && !report.dynasty_extinct
        {
            roll_monthly_event(sim, data, &mut report);
        }

        // Hard-stop the fast-forward the instant something needs attention — a
        // decision, a completion, an extinction, or crossing a phase boundary.
        if report.decision_required
            || report.contract_completed.is_some()
            || report.dynasty_extinct
            || report.phase_changed.is_some()
        {
            break;
        }
    }

    // Keep faction shares matched to the (possibly changed) head count (W7);
    // a faction rescaled to nothing is gone for good.
    for id in sim.rebalance_factions() {
        let name = crate::state::sim::factions::log_name(&data.factions, &id);
        sim.push_log(format!("The last of {name} is gone."));
    }

    sim.trim_log(data.config.log_limit);
    report
}

/// Test/tooling helper: advance one year's worth of the loop at `OneYear` speed.
/// Still hard-stops early on a decision, exactly like a player pressing Advance
/// at 1-yr speed.
pub fn advance_year(sim: &mut SimState, data: &GameData) -> TickReport {
    sim.speed = SpeedStep::OneYear;
    advance(sim, data)
}

/// One full economic year (GDD §5.1), applied on a year boundary: production,
/// food upkeep, wear, drift, generation/succession, contract progress, market.
/// Exactly the W1-tuned yearly math — only the clock advance and the (now
/// monthly) event roll live outside it (W3).
fn year_boundary_tick(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    let config = &data.config;

    // Production (GDD §5.1: floor(rate * years), one year per tick),
    // multiplied by the serving crew's skills (PLAN item 2).
    let crew_mult = crew::production_multipliers(sim, data);
    let produced = ResourceDelta {
        credits: (sim.production.credits * crew_mult.credits).floor() as i64,
        energy: (sim.production.energy * crew_mult.energy).floor() as i64,
        minerals: (sim.production.minerals * crew_mult.minerals).floor() as i64,
        food: (sim.production.food * crew_mult.food).floor() as i64,
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
    sim.ship.hull_integrity =
        (sim.ship.hull_integrity - config.hull_decay_per_year * wear).max(0.0);
    sim.ship.life_support =
        (sim.ship.life_support - config.life_support_decay_per_year * wear).max(0.0);

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

/// One month of contract progress (W2): objective accrual on-station, the
/// authored phase timeline, milestone payouts, and completion detection. Logs
/// milestones and phase crossings; surfaces a phase change and completion on
/// the report so the fast-forward can hard-stop.
fn month_of_contract(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    if sim.contract.is_none() {
        return;
    }
    let speed = ship::loadout_stats(sim, data).speed;
    let progress = contract::advance_contract(sim, &data.config, speed);
    for milestone in &progress.reached_milestones {
        sim.push_log(format!("Milestone reached: {milestone}"));
    }
    if let Some(phase) = progress.phase_changed {
        sim.push_log(contract::phase_transition_line(phase));
        report.phase_changed = Some(phase);
    }
    if let Some(result) = progress.completed {
        report.contract_completed = Some(result);
    }
}

fn roll_monthly_event(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    if let Some(pending) = event_resolver::roll_event(sim, data) {
        if let Some(template) = data.events.get(&pending.template_id).cloned() {
            let delegated = sim.delegation.is_delegated(template.category);
            if template.requires_decision && !delegated {
                sim.push_log(format!("Council decision required: {}", template.title));
                sim.pending_event = Some(pending);
                report.decision_required = true;
            } else {
                let label = event_resolver::auto_resolve(sim, data, &template);
                if delegated {
                    sim.push_log(format!(
                        "Delegated advisor resolved '{}' with: {label}",
                        template.title
                    ));
                }
            }
        }
    }
}

/// Apply one year of voyage drift to the population (PLAN M4.1). Identity terms
/// scale by the legacy's multiplier (Adaptors fastest, Preservers slowest); the
/// morale/unity strain is universal. Clamped to 0-1 by `PopulationState::apply`.
fn apply_voyage_drift(sim: &mut SimState, config: &GameConfig) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::simulation::contract::start_contract;

    fn fresh(seed: u64) -> (GameData, SimState) {
        let data = GameData::load().unwrap();
        let sim = SimState::new_campaign(
            &data,
            "preservers",
            seed,
            &crate::state::sim::founding_faction_ids(&data),
        );
        (data, sim)
    }

    #[test]
    fn voyage_drift_changes_the_people_and_stays_bounded() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let (a0, d0, l0) = (
            sim.population.adaptation,
            sim.population.cultural_drift,
            sim.population.legacy_loyalty,
        );
        // A long voyage with no events at all still reshapes the crew.
        for _ in 0..40 {
            apply_voyage_drift(&mut sim, &data.config);
        }
        assert!(sim.population.adaptation > a0, "adaptation rises underway");
        assert!(sim.population.cultural_drift > d0, "cultural drift rises");
        assert!(
            sim.population.legacy_loyalty < l0,
            "loyalty to the founders fades"
        );
        for v in [
            sim.population.adaptation,
            sim.population.cultural_drift,
            sim.population.legacy_loyalty,
            sim.population.morale,
            sim.population.unity,
        ] {
            assert!((0.0..=1.0).contains(&v), "drift stays a 0-1 fraction: {v}");
        }
    }

    #[test]
    fn a_neglected_generational_voyage_wears_the_ship_to_the_edge() {
        // Events off + well-fed isolates the wear curve (PLAN M4.2) for a
        // charter-length voyage flown with *no* field repairs — the neglect
        // baseline the autoplay repair policy is measured against (W1-rescale).
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;

        // Starting parts cover ~60 maintained years; after that the ship wears
        // at full rate. Over 300 years with no repair it comes home a wreck.
        for _ in 0..300 {
            advance_year(&mut sim, &data);
        }

        assert_eq!(
            sim.ship.spare_parts, 0,
            "a generational voyage long outlasts the spare-parts stores"
        );
        // Still nominally flying (hull > 0), but only just — held together on
        // hope and prayers, a hair from total loss. This is why the voyage
        // needs the field-repair sink the autoplay policy exercises.
        assert!(
            (0.0..=0.10).contains(&sim.ship.hull_integrity),
            "a neglected 300-year voyage should limp in near total loss: hull {}",
            sim.ship.hull_integrity
        );
    }

    #[test]
    fn voyage_drift_scales_by_legacy() {
        let data = GameData::load().unwrap();
        let mut adaptors = SimState::new_campaign(
            &data,
            "adaptors",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let mut preservers = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        for _ in 0..30 {
            apply_voyage_drift(&mut adaptors, &data.config);
            apply_voyage_drift(&mut preservers, &data.config);
        }
        assert!(
            adaptors.population.cultural_drift > preservers.population.cultural_drift,
            "Adaptors change faster than Preservers"
        );
    }

    #[test]
    fn a_year_produces_resources_and_consumes_food() {
        // Events off so the year runs to its boundary without a decision stop.
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            21,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let food_before = sim.resources.food;
        let credits_before = sim.resources.credits;

        let crew_mult = crate::simulation::crew::production_multipliers(&sim, &data);
        advance_year(&mut sim, &data);

        assert_eq!(sim.year(), 1);
        let upkeep =
            (sim.population.count as f32 * data.config.food_per_person_per_year).ceil() as i64;
        assert_eq!(
            sim.resources.food,
            food_before + (data.config.base_production.food * crew_mult.food).floor() as i64
                - upkeep
        );
        assert!(crew_mult.food > 1.0, "founding agronomist boosts food");
        assert!(sim.resources.credits >= credits_before);
        assert!(sim.ship.hull_integrity < 1.0);
    }

    #[test]
    fn identical_seeds_produce_identical_decades() {
        let (data, mut a) = fresh(77);
        let (_, mut b) = fresh(77);
        for _ in 0..10 {
            a.pending_event = None;
            a.pending_dilemma = None;
            b.pending_event = None;
            b.pending_dilemma = None;
            advance_year(&mut a, &data);
            advance_year(&mut b, &data);
        }
        assert_eq!(a.resources.credits, b.resources.credits);
        assert_eq!(a.population.count, b.population.count);
        assert_eq!(
            serde_json::to_string(&a.market.entries).unwrap(),
            serde_json::to_string(&b.market.entries).unwrap()
        );
        assert_eq!(a.log.len(), b.log.len());
    }

    #[test]
    fn contract_completes_at_target_duration() {
        // Events off isolates the timeline; advance_year now hard-stops on phase
        // boundaries too (W2), so loop to completion rather than a fixed count.
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        // Plenty of food so the population survives the run deterministically.
        sim.resources.food = 1_000_000;

        let mut completed = None;
        // Each advance_year covers up to a year; the cap comfortably exceeds the
        // calls needed to reach target_duration_years * 12 months.
        for _ in 0..(template.target_duration_years * 12) {
            let report = advance_year(&mut sim, &data);
            if report.contract_completed.is_some() {
                completed = report.contract_completed;
                break;
            }
        }
        let (score, _) = completed.expect("contract must complete at its target duration");
        assert!(score > 0.0);
        let active = sim.contract.as_ref().unwrap();
        assert_eq!(
            active.months_elapsed,
            template.target_duration_years * 12,
            "completes exactly at the authored duration"
        );
        assert!(active.milestones.iter().all(|m| m.reached));
    }

    #[test]
    fn a_phase_boundary_hard_stops_the_fast_forward() {
        // Events off + a fresh charter: a 10-yr advance departs and hard-stops
        // on the very first phase crossing (Preparation → Travel) after 1 month.
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            9,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        sim.resources.food = 1_000_000;
        sim.speed = SpeedStep::TenYears;

        let report = advance(&mut sim, &data);
        assert_eq!(report.months_advanced, 1, "departure is a hard-stop");
        assert_eq!(
            report.phase_changed,
            Some(crate::data::contracts::ContractPhase::Travel)
        );
        assert_eq!(sim.contract.as_ref().unwrap().phase, ContractPhase::Travel);
    }

    #[test]
    fn a_certain_dilemma_fires_on_the_generation_boundary() {
        // Events off isolates the generation dilemma as the only decision.
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 1.0;
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            11,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;

        for _ in 0..data.config.generation_interval_years {
            advance_year(&mut sim, &data);
        }
        let pending = sim
            .pending_dilemma
            .as_ref()
            .expect("a dilemma must confront the new generation at 100% chance");
        assert_eq!(pending.rolled_month_clock, sim.month_clock);
        // The dilemma blocks the month's event roll — one decision at a time.
        assert!(sim.pending_event.is_none());
    }

    #[test]
    fn a_ten_year_advance_matches_ten_one_year_advances() {
        // Events off isolates the deterministic economic path so the two
        // cadences must land byte-for-byte on the same state.
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;

        let mut fast = SimState::new_campaign(
            &data,
            "preservers",
            123,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let mut slow = SimState::new_campaign(
            &data,
            "preservers",
            123,
            &crate::state::sim::founding_faction_ids(&data),
        );
        fast.resources.food = 1_000_000;
        slow.resources.food = 1_000_000;

        fast.speed = SpeedStep::TenYears;
        let report = advance(&mut fast, &data);
        assert_eq!(
            report.months_advanced, 120,
            "a clear 10-yr advance crosses exactly 120 months"
        );
        assert_eq!(fast.month_clock, 120);
        assert_eq!(fast.year(), 10);

        for _ in 0..10 {
            advance_year(&mut slow, &data);
        }
        assert_eq!(fast.month_clock, slow.month_clock);
        assert_eq!(fast.resources.credits, slow.resources.credits);
        assert_eq!(fast.population.count, slow.population.count);
        assert_eq!(
            fast.ship.hull_integrity.to_bits(),
            slow.ship.hull_integrity.to_bits(),
            "10 boundary ticks either way leave identical hull wear"
        );
    }

    #[test]
    fn a_fast_advance_stops_at_the_generation_dilemma() {
        // Short generations + a certain dilemma + events off: a 10-yr press must
        // stop dead on the first generation boundary, not run the full 120.
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 1.0;
        data.config.generation_interval_years = 5;

        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            11,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;
        sim.speed = SpeedStep::TenYears;

        let report = advance(&mut sim, &data);
        assert!(
            sim.pending_dilemma.is_some(),
            "the generation dilemma must block the fast-forward"
        );
        assert_eq!(
            report.months_advanced, 60,
            "stopped on the year-5 boundary, not the full 120 months"
        );
        assert!(report.months_advanced < 120);
        assert_eq!(sim.year(), 5);
    }

    #[test]
    fn a_fired_event_is_dated_in_the_log() {
        // Force an event every month (no dilemmas) so a blocking one lands fast;
        // its pending date must match a stamped log line (W3).
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 1.0;
        data.config.event_chance_cap = 1.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;

        // Advance until a council-blocking event is pending.
        for _ in 0..40 {
            if sim.pending_event.is_some() {
                break;
            }
            advance_year(&mut sim, &data);
        }
        let pending = sim
            .pending_event
            .clone()
            .expect("a blocking event should fire under a certain event chance");
        let year = pending.rolled_month_clock / 12;
        let month = pending.rolled_month_clock % 12 + 1;
        assert!(
            sim.log.iter().any(|e| e.year == year && e.month == month),
            "the fired event must leave a log line dated Y{year}·M{month:02}"
        );
    }
}
