//! Automated full-mission playthrough harness (W1-rescale).
//!
//! The owner's primary playtest channel: a deterministic *policy player* that
//! starts a charter and flies it year by year with a fixed, dumb strategy —
//! resolve every council decision by first choice, patch the hull when it
//! slips, buy food when the stores run low — then reports how the voyage
//! ended. It exists to soak the whole content set (events, dilemmas,
//! succession, contract completion) across a generational voyage and catch any
//! invariant that escapes its range along the way.
//!
//! Test-only: it drives the same stateless services the game does, so a green
//! soak means a real campaign of that length stays internally consistent.

use crate::data::GameData;
use crate::simulation::contract::start_contract;
use crate::simulation::tick::advance;
use crate::simulation::{event_resolver, legacy, market, ship};
use crate::state::sim::{SimState, SpeedStep, TradeResource};

/// How a played-out mission ended.
#[derive(Debug, Clone, Copy)]
pub struct MissionOutcome {
    /// The charter reached its target duration and scored out.
    pub completed: bool,
    /// The dynasty ran out of heirs before the charter concluded.
    pub extinct: bool,
    /// The campaign year the run ended on.
    pub final_year: u32,
    /// Success score at completion (0.0 if the run never completed).
    pub final_score: f32,
}

/// Fly `contract_id` to its conclusion (or `max_years`, whichever comes first)
/// under a fixed policy, asserting every per-year invariant along the way.
///
/// Policy: resolve any pending dilemma/event by first choice (index 0); field-
/// repair the hull whenever it drops below half and the parts/minerals are
/// there; buy a batch of food whenever the stores fall under the crisis
/// threshold and credits allow. Deterministic for a given (sim, contract)
/// pair — all randomness flows through `sim.rng`.
pub fn play_mission(
    sim: &mut SimState,
    data: &GameData,
    contract_id: &str,
    max_years: u32,
) -> MissionOutcome {
    let template = data
        .contracts
        .get(contract_id)
        .expect("autoplay contract id must resolve to a charter")
        .clone();
    // Provision and launch explicitly (W4): top the tank in port, put the
    // charter under consideration, then commit — no silent contract start.
    sim.ship.fuel = 1.0;
    sim.selected_charter = Some(contract_id.to_owned());
    sim.contract = Some(start_contract(&template, sim));
    sim.selected_charter = None;
    // Fly at full speed: each Advance covers up to a decade but hard-stops on
    // the next decision, so the policy still resolves everything in order (W3).
    sim.speed = SpeedStep::TenYears;

    let mut outcome = MissionOutcome {
        completed: false,
        extinct: false,
        final_year: sim.year(),
        final_score: 0.0,
    };

    let max_months = max_years * 12;
    // Once a faction has left the ship it must never reappear as Aboard (W7).
    let mut ever_lost: std::collections::HashSet<String> = std::collections::HashSet::new();
    while sim.month_clock < max_months {
        // Clear any blocking council decision by taking the first choice — the
        // same dumb policy the game's own soak has always used.
        if sim.pending_dilemma.is_some() {
            legacy::resolve_dilemma(sim, data, 0);
        }
        if let Some(pending) = sim.pending_event.clone() {
            match data.events.get(&pending.template_id).cloned() {
                Some(t) => event_resolver::apply_outcome(sim, data, &t, 0),
                None => sim.pending_event = None,
            }
        }
        if sim.dynasty.extinct {
            outcome.extinct = true;
            break;
        }

        // Standing orders: keep the hull off the floor and the galley stocked.
        // Both verbs refuse (harmlessly) when they can't be paid for.
        if sim.ship.hull_integrity < 0.5 {
            let _ = ship::field_repair(sim, &data.config, ship::RepairKind::Hull);
        }
        if sim.resources.food < data.config.low_food_threshold {
            let _ = market::buy(sim, TradeResource::Food, 1000);
        }

        let report = advance(sim, data);
        outcome.final_year = sim.year();
        assert_year_invariants(sim);
        for faction in &sim.factions {
            if faction.is_aboard() {
                assert!(
                    !ever_lost.contains(&faction.faction_id),
                    "a lost faction returned to Aboard: {}",
                    faction.faction_id
                );
            } else {
                ever_lost.insert(faction.faction_id.clone());
            }
        }

        if let Some((score, _)) = report.contract_completed {
            outcome.completed = true;
            outcome.final_score = score;
            sim.contract = None;
            break;
        }
        if report.dynasty_extinct {
            outcome.extinct = true;
            break;
        }
    }

    outcome
}

/// Every invariant that must hold at the end of any simulated year: 0-1
/// fractions stay in range, resources never go negative, and a living dynasty
/// always has someone at its head.
fn assert_year_invariants(sim: &SimState) {
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
            sim.year()
        );
    }
    assert!(sim.resources.food >= 0 && sim.resources.credits >= 0);
    if !sim.dynasty.extinct {
        assert!(
            sim.dynasty.leader().is_some(),
            "a living dynasty must always have a leader (year {})",
            sim.year()
        );
    }
    // W7: Aboard members always sum to the head count, and a faction that has
    // left the ship carries no members.
    let aboard_sum: u32 = sim
        .factions
        .iter()
        .filter(|f| f.is_aboard())
        .map(|f| f.members)
        .sum();
    assert_eq!(
        aboard_sum,
        sim.population.count,
        "faction members must sum to population.count (year {})",
        sim.year()
    );
    for faction in &sim.factions {
        if !faction.is_aboard() {
            assert_eq!(
                faction.members, 0,
                "a departed faction carries no members ({})",
                faction.faction_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Soak test: fly the 340-year Deep Vein Survey end to end under the
    /// autoplay policy. It must conclude with the charter completed and the
    /// dynasty still alive, twelve-plus generations on. Per-year invariants are
    /// asserted inside `play_mission`.
    #[test]
    fn deep_vein_survey_completes_with_a_living_dynasty() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            2024,
            &crate::state::sim::founding_faction_ids(&data),
        );

        let outcome = play_mission(&mut sim, &data, "deep_vein_survey", 420);

        assert!(
            outcome.completed,
            "the 340-year survey should complete under autoplay (ended year {}, extinct {})",
            outcome.final_year, outcome.extinct
        );
        assert!(
            !outcome.extinct,
            "the dynasty should survive the survey with the autoplay policy"
        );
        assert!(
            sim.dynasty.generation >= 12,
            "340 years is 12+ successions; dynasty only reached generation {}",
            sim.dynasty.generation
        );
    }

    /// The 600-year Long Dark is allowed to end either way: completion or the
    /// total loss of the line. This test only pins the invariants (asserted in
    /// `play_mission`) and that the run terminates in one of the two legal
    /// outcomes rather than running the clock out mid-voyage.
    #[test]
    fn the_long_dark_ends_in_completion_or_extinction() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            7,
            &crate::state::sim::founding_faction_ids(&data),
        );

        let outcome = play_mission(&mut sim, &data, "the_long_dark", 700);

        assert!(
            outcome.completed || outcome.extinct,
            "the long dark should resolve to completion or extinction, not run out the clock \
             (year {}, completed {}, extinct {})",
            outcome.final_year,
            outcome.completed,
            outcome.extinct
        );
    }

    /// Turning back at year 150 banks only part of the objective, and pay is
    /// strictly proportional — a truncated run earns less than a full term (W2).
    #[test]
    fn aborting_at_year_150_reduces_the_pay() {
        use crate::simulation::contract::{advance_contract, jump_to_return, prorated_reward};

        let data = GameData::load().unwrap();
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();

        // Fly the contract clock straight through — no economy needed to measure
        // the objective the timeline banks.
        let full_fraction = {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                2024,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.contract = Some(start_contract(&template, &sim));
            let total = sim.contract.as_ref().unwrap().total_months();
            while sim.contract.as_ref().unwrap().months_elapsed < total {
                advance_contract(&mut sim, &data.config, 0);
            }
            sim.contract.as_ref().unwrap().objective_fraction()
        };
        assert!(full_fraction >= 0.99, "a full term meets the objective");

        let abort_fraction = {
            let mut sim = SimState::new_campaign(
                &data,
                "preservers",
                2024,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.contract = Some(start_contract(&template, &sim));
            while sim.contract.as_ref().unwrap().months_elapsed < 150 * 12 {
                advance_contract(&mut sim, &data.config, 0);
            }
            assert!(jump_to_return(&mut sim), "turning back at year 150");
            let total = sim.contract.as_ref().unwrap().total_months();
            while sim.contract.as_ref().unwrap().months_elapsed < total {
                advance_contract(&mut sim, &data.config, 0);
            }
            sim.contract.as_ref().unwrap().objective_fraction()
        };

        assert!(
            abort_fraction > 0.0 && abort_fraction < full_fraction,
            "aborting at year 150 banks some but not all of the objective: {abort_fraction} vs {full_fraction}"
        );
        let full_pay = prorated_reward(&template.reward, full_fraction);
        let abort_pay = prorated_reward(&template.reward, abort_fraction);
        assert!(
            abort_pay.credits < full_pay.credits,
            "a truncated mission pays less: {} vs {}",
            abort_pay.credits,
            full_pay.credits
        );
    }

    /// A launch on a dry tank strands the ship in transit (W4): it stalls, so
    /// after the same calendar span its contract has advanced measurably less
    /// than a fully-fuelled run's, and it logged stalled months.
    #[test]
    fn an_under_fuelled_launch_stalls_and_falls_behind() {
        let mut data = GameData::load().unwrap();
        data.config.event_chance_base = 0.0;
        data.config.event_chance_cap = 0.0;
        data.config.dilemma_chance_per_generation = 0.0;
        let picks = crate::state::sim::founding_faction_ids(&data);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();

        // Returns (calendar months, contract months, stalled months).
        let run = |fuel: f32| -> (u32, u32, u32) {
            let mut sim = SimState::new_campaign(&data, "preservers", 7, &picks);
            sim.contract = Some(start_contract(&template, &sim));
            sim.resources.food = 10_000_000;
            sim.ship.fuel = fuel;
            sim.speed = SpeedStep::TenYears;
            while sim.month_clock < 50 * 12 {
                advance(&mut sim, &data);
            }
            (
                sim.month_clock,
                sim.contract.as_ref().unwrap().months_elapsed,
                sim.stalled_months,
            )
        };

        let (fuelled_cal, fuelled_con, fuelled_stall) = run(1.0);
        let (dry_cal, dry_con, dry_stall) = run(0.0);

        assert_eq!(fuelled_stall, 0, "a full tank never stalls in transit");
        assert_eq!(
            fuelled_cal, fuelled_con,
            "a fuelled voyage's calendar keeps pace with its contract clock"
        );
        assert!(dry_stall > 0, "a dry launch strands the ship");
        assert!(
            dry_cal > dry_con,
            "the dry run's calendar outran its contract clock: {dry_cal} > {dry_con}"
        );
        assert_eq!(
            dry_cal - dry_con,
            dry_stall,
            "the calendar/contract gap is exactly the stalled months"
        );
    }
}
