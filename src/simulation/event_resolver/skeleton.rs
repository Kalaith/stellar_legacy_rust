//! Seeded campaign skeleton (W6): the major beats of a mission, laid out at
//! LAUNCH from the mission seed so a centuries-long voyage reads as a generated
//! campaign rather than a random-event stream. Same seed ⇒ same schedule.
//!
//! The families themselves are authored content (JSON); only the *pool
//! structure* — which families belong to which phase — is mechanics, and lives
//! here as a constant table.

use crate::data::contracts::ContractPhase;
use crate::data::CampaignSkeletonConfig;
use crate::state::sim::{ActiveContract, CampaignBeat};
use macroquad_toolkit::rng::SeededRng;

fn pool_for_phase(cfg: &CampaignSkeletonConfig, phase: ContractPhase) -> &[String] {
    match phase {
        ContractPhase::Travel | ContractPhase::Preparation => &cfg.travel_pool,
        ContractPhase::Operation => &cfg.operation_pool,
        ContractPhase::Return | ContractPhase::Completion => &cfg.return_pool,
    }
}

/// Lay out the campaign beats for `contract` (W6): one beat per full
/// `months_per_window` of mission duration, each placed uniformly at random
/// within its own window (skipping the first `skip_months` overall), drawing a
/// family from the phase pool active at that month, the any-phase families, and
/// — depending where in the voyage the beat lands — the founding-era or
/// homecoming-era pool (content-depth era layering). Deterministic for a given
/// rng state.
pub fn generate_beats(
    rng: &mut SeededRng,
    contract: &ActiveContract,
    cfg: &CampaignSkeletonConfig,
) -> Vec<CampaignBeat> {
    let total_months = contract.total_months();
    let windows = total_months / cfg.months_per_window;
    let early_cutoff = (total_months as f32 * cfg.early_fraction) as u32;
    let late_cutoff = (total_months as f32 * cfg.late_fraction) as u32;
    let mut beats = Vec::with_capacity(windows as usize);
    for i in 0..windows {
        let window_start = i * cfg.months_per_window;
        let lo = window_start.max(cfg.skip_months);
        let hi = window_start + cfg.months_per_window;
        if lo >= hi {
            continue;
        }
        let month = lo + rng.below((hi - lo) as usize) as u32;
        let (_, phase) = contract.phase_at(month + 1);
        // Build the eligible draw for this beat: phase pool + any-phase, plus the
        // era pool for where it lands. Order is deterministic, so a fixed rng
        // state yields a fixed schedule.
        let mut draw: Vec<&str> = pool_for_phase(cfg, phase)
            .iter()
            .chain(cfg.any_pool.iter())
            .map(String::as_str)
            .collect();
        if month < early_cutoff {
            draw.extend(cfg.early_pool.iter().map(String::as_str));
        } else if month < late_cutoff {
            // The deep middle: neither founding nor homecoming, tinted by the
            // era no one aboard remembers beginning (content-depth round 4).
            draw.extend(cfg.mid_pool.iter().map(String::as_str));
        }
        if month >= late_cutoff {
            draw.extend(cfg.late_pool.iter().map(String::as_str));
        }
        let family = draw[rng.below(draw.len())].to_owned();
        beats.push(CampaignBeat {
            month_clock: month,
            family,
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

        let cfg = &data.config.campaign_skeleton;
        let schedule = || {
            let mut sim = SimState::new_campaign(&data, "preservers", 99, &picks);
            let contract = start_contract(&template, &sim);
            generate_beats(&mut sim.rng, &contract, cfg)
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

        // Beats are ordered, skip the opening window, and only draw families the
        // config declares (phase pools + any-phase + both era pools).
        let valid: std::collections::HashSet<&str> = cfg
            .travel_pool
            .iter()
            .chain(&cfg.operation_pool)
            .chain(&cfg.return_pool)
            .chain(&cfg.any_pool)
            .chain(&cfg.early_pool)
            .chain(&cfg.mid_pool)
            .chain(&cfg.late_pool)
            .map(String::as_str)
            .collect();
        for beat in &a {
            assert!(
                beat.month_clock >= cfg.skip_months,
                "no beat before the skip window"
            );
            assert!(valid.contains(beat.family.as_str()));
        }
    }

    #[test]
    fn era_layering_tints_the_ends_of_a_voyage() {
        let data = GameData::load().unwrap();
        let cfg = &data.config.campaign_skeleton;
        // Founding-, mid-, and homecoming-era pools must be authored for the
        // layering to mean anything, and must be real event families.
        assert!(!cfg.early_pool.is_empty() && !cfg.late_pool.is_empty());
        assert!(
            !cfg.mid_pool.is_empty(),
            "the deep middle needs its own tint"
        );
        for fam in cfg
            .early_pool
            .iter()
            .chain(&cfg.mid_pool)
            .chain(&cfg.late_pool)
        {
            assert!(
                data.events.iter().any(|(_, e)| &e.family == fam),
                "era family '{fam}' has no events"
            );
        }
    }
}
