//! The yearly simulation tick (GDD §3 step 3, §5.1).
//!
//! Time advances only on explicit player action (Pillar 4). One call =
//! one in-game year: production, upkeep, wear, aging, contract progress,
//! market drift, then an event roll.

use crate::data::{GameConfig, GameData, PopulationDelta, ResourceDelta};
use crate::simulation::contract::{score_success, SuccessLevel};
use crate::simulation::{contract, crew, event_resolver, legacy, market, ship, succession};
use crate::state::sim::SimState;

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
}

pub fn advance_year(sim: &mut SimState, data: &GameData) -> TickReport {
    debug_assert!(
        !sim.has_pending_decision(),
        "caller must resolve the pending event/dilemma before advancing time"
    );
    let config = &data.config;
    let mut report = TickReport::default();

    sim.year += 1;

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

    // Contract progress and completion (GDD §5.2). Ship speed adds bonus
    // progress (PLAN item 3).
    let contract_speed = ship::loadout_stats(sim, data).speed;
    for milestone in contract::advance_contract(sim, config, contract_speed) {
        sim.push_log(format!("Milestone reached: {milestone}"));
    }
    if let Some(active) = &sim.contract {
        if active.years_elapsed >= active.target_duration_years {
            report.contract_completed = Some(score_success(&active.metrics));
        }
    }

    // Market drift.
    market::drift_prices(sim);

    // Event roll (GDD §5.4). Delegated or no-decision events resolve
    // immediately but still log their outcome (GDD §3 step 4). A dilemma
    // rolled this year already blocks the council — one decision per year.
    if sim.pending_dilemma.is_none() {
        roll_yearly_event(sim, data, &mut report);
    }

    sim.trim_log(config.log_limit);
    report
}

fn roll_yearly_event(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
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
        let sim = SimState::new_campaign(&data, "preservers", seed);
        (data, sim)
    }

    #[test]
    fn voyage_drift_changes_the_people_and_stays_bounded() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "wanderers", 1);
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
    fn a_long_voyage_leaves_the_ship_worn_and_out_of_parts() {
        // Events off + well-fed isolates the wear curve (PLAN M4.2).
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let mut sim = SimState::new_campaign(&data, "preservers", 5);
        sim.resources.food = 1_000_000;

        for _ in 0..55 {
            advance_year(&mut sim, &data);
        }

        assert_eq!(
            sim.ship.spare_parts, 0,
            "a 55-year voyage exhausts the spare-parts stores"
        );
        // Worn but still flying — well below the old 0.005/yr curve (~0.72).
        assert!(
            (0.40..=0.56).contains(&sim.ship.hull_integrity),
            "the ship comes home held together on hope and prayers: hull {}",
            sim.ship.hull_integrity
        );
    }

    #[test]
    fn voyage_drift_scales_by_legacy() {
        let data = GameData::load().unwrap();
        let mut adaptors = SimState::new_campaign(&data, "adaptors", 1);
        let mut preservers = SimState::new_campaign(&data, "preservers", 1);
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
        let (data, mut sim) = fresh(21);
        let food_before = sim.resources.food;
        let credits_before = sim.resources.credits;
        sim.pending_event = None;

        let crew_mult = crate::simulation::crew::production_multipliers(&sim, &data);
        advance_year(&mut sim, &data);

        assert_eq!(sim.year, 1);
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
        let (data, mut sim) = fresh(5);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        // Plenty of food so the population survives the run deterministically.
        sim.resources.food = 1_000_000;

        let mut completed = None;
        for _ in 0..template.target_duration_years {
            sim.pending_event = None;
            sim.pending_dilemma = None;
            let report = advance_year(&mut sim, &data);
            if report.contract_completed.is_some() {
                completed = report.contract_completed;
                break;
            }
        }
        let (score, _) = completed.expect("contract must complete at its target duration");
        assert!(score > 0.0);
        let active = sim.contract.as_ref().unwrap();
        assert!(active.milestones.iter().all(|m| m.reached));
    }

    #[test]
    fn ship_speed_adds_bonus_contract_progress() {
        let (data, mut sim) = fresh(9);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        sim.resources.food = 1_000_000;
        sim.pending_event = None;
        sim.pending_dilemma = None;

        advance_year(&mut sim, &data);

        let contract = sim.contract.as_ref().unwrap();
        assert!(
            contract.bonus_progress > 0.0,
            "the founding loadout's speed should add bonus progress"
        );
        // Progress outruns the naive years/duration thanks to the speed bonus.
        let naive = contract.years_elapsed as f32 / contract.target_duration_years as f32;
        assert!(contract.progress() > naive);
    }

    #[test]
    fn a_certain_dilemma_fires_on_the_generation_boundary() {
        let mut data = GameData::load().unwrap();
        data.config.dilemma_chance_per_generation = 1.0;
        let mut sim = SimState::new_campaign(&data, "preservers", 11);
        sim.resources.food = 1_000_000;

        for _ in 0..data.config.generation_interval_years {
            sim.pending_event = None;
            sim.pending_dilemma = None;
            advance_year(&mut sim, &data);
        }
        let pending = sim
            .pending_dilemma
            .as_ref()
            .expect("a dilemma must confront the new generation at 100% chance");
        assert_eq!(pending.rolled_year, sim.year);
        // The dilemma blocks the year's event roll — one decision at a time.
        assert!(sim.pending_event.is_none());
    }

    /// Soak test: run a well-fed campaign across many generations, resolving
    /// every council decision, and assert the sim stays internally consistent
    /// the whole way. Exercises the full content set (events, dilemmas,
    /// succession, contract completion) end-to-end for regression safety.
    #[test]
    fn long_campaign_stays_internally_consistent() {
        let (data, mut sim) = fresh(2024);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        sim.resources.food = 10_000_000; // survive long enough to soak the loop

        let mut contract_completed = false;
        for _ in 0..250 {
            // Clear any blocking council decision by taking the first choice.
            if sim.pending_dilemma.is_some() {
                crate::simulation::legacy::resolve_dilemma(&mut sim, &data, 0);
            }
            if let Some(pending) = sim.pending_event.clone() {
                match data.events.get(&pending.template_id).cloned() {
                    Some(t) => crate::simulation::event_resolver::apply_outcome(&mut sim, &t, 0),
                    None => sim.pending_event = None,
                }
            }
            if sim.dynasty.extinct {
                break;
            }

            let report = advance_year(&mut sim, &data);
            if report.contract_completed.is_some() {
                contract_completed = true;
                sim.contract = None;
            }

            // Invariants that must hold every single year.
            for fraction in [
                sim.population.morale,
                sim.population.unity,
                sim.population.stability,
                sim.population.legacy_loyalty,
                sim.population.adaptation,
                sim.population.cultural_drift,
                sim.ship.hull_integrity,
                sim.ship.life_support,
                sim.ship.fuel,
            ] {
                assert!(
                    (0.0..=1.0).contains(&fraction),
                    "0-1 sim fraction escaped its range: {fraction} at year {}",
                    sim.year
                );
            }
            assert!(sim.resources.food >= 0 && sim.resources.credits >= 0);
            if !sim.dynasty.extinct {
                assert!(
                    sim.dynasty.leader().is_some(),
                    "a living dynasty must always have a leader (year {})",
                    sim.year
                );
            }
        }

        assert!(
            contract_completed,
            "the 40-year survey should conclude within a 250-year soak"
        );
    }
}
