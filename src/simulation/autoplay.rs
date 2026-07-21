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
use crate::simulation::tick::advance_year;
use crate::simulation::{event_resolver, legacy, market, ship};
use crate::state::sim::{SimState, TradeResource};

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
    sim.contract = Some(start_contract(&template, sim));

    let mut outcome = MissionOutcome {
        completed: false,
        extinct: false,
        final_year: sim.year,
        final_score: 0.0,
    };

    for _ in 0..max_years {
        // Clear any blocking council decision by taking the first choice — the
        // same dumb policy the game's own soak has always used.
        if sim.pending_dilemma.is_some() {
            legacy::resolve_dilemma(sim, data, 0);
        }
        if let Some(pending) = sim.pending_event.clone() {
            match data.events.get(&pending.template_id).cloned() {
                Some(t) => event_resolver::apply_outcome(sim, &t, 0),
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

        let report = advance_year(sim, data);
        outcome.final_year = sim.year;
        assert_year_invariants(sim);

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
        let mut sim = SimState::new_campaign(&data, "preservers", 2024);

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
        let mut sim = SimState::new_campaign(&data, "wanderers", 7);

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
}
