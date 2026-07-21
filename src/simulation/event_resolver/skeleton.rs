//! Seeded campaign skeleton (W6): the major beats of a mission, laid out at
//! LAUNCH from the mission seed so a centuries-long voyage reads as a generated
//! campaign rather than a random-event stream. Same seed ⇒ same schedule.
//!
//! The families themselves are authored content (JSON); only the *pool
//! structure* — which families belong to which phase — is mechanics, and lives
//! here as a constant table.

use crate::data::contracts::ContractPhase;
use crate::state::sim::{ActiveContract, CampaignBeat};
use macroquad_toolkit::rng::SeededRng;

/// Families a Travel-phase beat may draw from.
const TRAVEL_POOL: &[&str] = &[
    "exploration_first_contact",
    "science_anomaly",
    "diplomacy",
    "mystery",
    "engineering",
];
/// Families an Operation-phase beat may draw from.
const OPERATION_POOL: &[&str] = &["survival", "diplomacy", "engineering", "mystery"];
/// Families a Return-phase beat may draw from.
const RETURN_POOL: &[&str] = &["legacy_drift", "ethics", "mystery"];
/// Families allowed in any phase, always added to the draw.
const ANY_POOL: &[&str] = &["biology_medical", "comedy"];

const MONTHS_PER_WINDOW: u32 = 20 * 12;
const SKIP_MONTHS: u32 = 5 * 12;

fn pool_for_phase(phase: ContractPhase) -> &'static [&'static str] {
    match phase {
        ContractPhase::Travel | ContractPhase::Preparation => TRAVEL_POOL,
        ContractPhase::Operation => OPERATION_POOL,
        ContractPhase::Return | ContractPhase::Completion => RETURN_POOL,
    }
}

/// Lay out the campaign beats for `contract` (W6): one beat per full 20 years of
/// mission duration, each placed uniformly at random within its own 20-year
/// window (skipping the first 5 years overall), drawing a family from the pool
/// for the phase active at that month plus the any-phase families. Deterministic
/// for a given rng state.
pub fn generate_beats(rng: &mut SeededRng, contract: &ActiveContract) -> Vec<CampaignBeat> {
    let total_months = contract.total_months();
    let windows = total_months / MONTHS_PER_WINDOW;
    let mut beats = Vec::with_capacity(windows as usize);
    for i in 0..windows {
        let window_start = i * MONTHS_PER_WINDOW;
        let lo = window_start.max(SKIP_MONTHS);
        let hi = window_start + MONTHS_PER_WINDOW;
        if lo >= hi {
            continue;
        }
        let month = lo + rng.below((hi - lo) as usize) as u32;
        let (_, phase) = contract.phase_at(month + 1);
        let pool = pool_for_phase(phase);
        let idx = rng.below(pool.len() + ANY_POOL.len());
        let family = if idx < pool.len() {
            pool[idx]
        } else {
            ANY_POOL[idx - pool.len()]
        };
        beats.push(CampaignBeat {
            month_clock: month,
            family: family.to_owned(),
            fired: false,
        });
    }
    beats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::simulation::contract::start_contract;
    use crate::state::sim::{founding_faction_ids, SimState};

    #[test]
    fn beats_are_deterministic_and_one_per_twenty_years() {
        let data = GameData::load().unwrap();
        let picks = founding_faction_ids(&data);
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();

        let schedule = || {
            let mut sim = SimState::new_campaign(&data, "preservers", 99, &picks);
            let contract = start_contract(&template, &sim);
            generate_beats(&mut sim.rng, &contract)
        };
        let a = schedule();
        let b = schedule();

        // A 340-year charter spans 17 twenty-year windows → 17 beats.
        assert_eq!(
            a.len(),
            17,
            "one beat per full 20 years of a 340-yr charter"
        );
        let flat = |v: &[CampaignBeat]| -> Vec<(u32, String)> {
            v.iter()
                .map(|x| (x.month_clock, x.family.clone()))
                .collect()
        };
        assert_eq!(flat(&a), flat(&b), "same seed replays the same schedule");

        // Beats are ordered, skip the first five years, and only draw valid
        // phase-appropriate families.
        let valid: std::collections::HashSet<&str> = TRAVEL_POOL
            .iter()
            .chain(OPERATION_POOL)
            .chain(RETURN_POOL)
            .chain(ANY_POOL)
            .copied()
            .collect();
        for beat in &a {
            assert!(
                beat.month_clock >= SKIP_MONTHS,
                "no beat in the first 5 years"
            );
            assert!(valid.contains(beat.family.as_str()));
        }
    }
}
