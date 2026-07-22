//! Legacy dilemmas and the failure-risk formula (GDD §5.5).
//!
//! Dilemmas fire on generation boundaries: each new generation confronts the
//! legacy's defining tension. Their success/failure branches update the real
//! tracked counters on `LegacyTrack`, which in turn feed the failure-risk
//! score surfaced on the Crew & Dynasty screen.

use crate::data::legacies::{DilemmaDef, DilemmaEffect, DilemmaOption};
use crate::data::{GameConfig, GameData};
use crate::state::sim::{PendingDilemma, SimState};

/// Roll whether the new generation faces a legacy dilemma. Called from the
/// tick on generation boundaries only; returns the pending dilemma without
/// applying anything (dilemmas always block — they are the player's defining
/// choice and are never delegated).
pub fn roll_dilemma(sim: &mut SimState, data: &GameData) -> Option<PendingDilemma> {
    if !sim.rng.chance(data.config.dilemma_chance_per_generation) {
        return None;
    }
    let legacy = data.legacies.get(&sim.legacy.legacy_id)?;
    if legacy.dilemmas.is_empty() {
        return None;
    }
    let dilemma = &legacy.dilemmas[sim.rng.below(legacy.dilemmas.len())];
    Some(PendingDilemma {
        dilemma_id: dilemma.id.clone(),
        rolled_month_clock: sim.month_clock,
    })
}

/// Look up the sim's pending dilemma definition in the loaded data.
pub fn pending_dilemma_def<'a>(sim: &SimState, data: &'a GameData) -> Option<&'a DilemmaDef> {
    let pending = sim.pending_dilemma.as_ref()?;
    data.legacies
        .get(&sim.legacy.legacy_id)?
        .dilemmas
        .iter()
        .find(|d| d.id == pending.dilemma_id)
}

/// Effective success chance for a dilemma option: the base chance plus a
/// combat bonus on Wanderer dilemmas (firepower backs the confrontation —
/// GDD combat → wanderer odds), capped by config. Shown honestly in the modal
/// and used for the roll (Pillar 3).
pub fn dilemma_odds(sim: &SimState, data: &GameData, option: &DilemmaOption) -> f32 {
    let combat_bonus = if sim.legacy.legacy_id == "wanderers" {
        let combat = crate::simulation::ship::loadout_stats(sim, data).combat;
        combat as f32 * data.config.ship.combat_dilemma_odds_per_point
    } else {
        0.0
    };
    // Who runs the ship can back or hinder a defining gamble (content-depth
    // factions round 10): while the named faction is dominant, its craft (or its
    // resistance) shifts the option's odds — the augmented back an augmentation,
    // the makers a risky repair, the arbiters drag on summary justice.
    let faction_bonus = if !option.dominant_faction.is_empty()
        && sim.dominant_faction_id() == Some(option.dominant_faction.as_str())
    {
        option.dominant_faction_odds
    } else {
        0.0
    };
    (option.success_chance + combat_bonus + faction_bonus)
        .clamp(0.0, data.config.ship.dilemma_odds_cap)
}

/// Resolve the pending dilemma with the chosen option: roll the option's
/// (combat-adjusted) success chance on the sim RNG, apply the winning branch
/// (including the legacy counters), log it, and clear the pending state.
/// Returns the log line that was recorded.
pub fn resolve_dilemma(sim: &mut SimState, data: &GameData, option_index: usize) -> Option<String> {
    let dilemma = pending_dilemma_def(sim, data)?.clone();
    let option = dilemma.options.get(option_index)?;

    let chance = dilemma_odds(sim, data, option);
    let succeeded = sim.rng.chance(chance);
    let effect = if succeeded {
        option.success.clone()
    } else {
        option.failure.clone()
    };

    apply_dilemma_effect(sim, &effect);
    let text = if effect.log.is_empty() {
        format!("{}: {}", dilemma.title, option.label)
    } else {
        effect.log.clone()
    };
    sim.push_log(text.clone());
    sim.pending_dilemma = None;
    Some(text)
}

fn apply_dilemma_effect(sim: &mut SimState, effect: &DilemmaEffect) {
    sim.resources.apply(&effect.resource_delta);
    sim.ship.apply(&effect.ship_delta);
    sim.population.apply(&effect.population_delta);

    let track = &mut sim.legacy;
    track.tradition_points += effect.tradition_points;
    track.body_horror_events += effect.body_horror_events;
    track.existential_dread = (track.existential_dread + effect.existential_dread).clamp(0.0, 1.0);
    track.piracy_reputation = (track.piracy_reputation + effect.piracy_reputation).clamp(0.0, 1.0);
}

/// One contributing factor of the failure-risk score, for honest UI display
/// (Pillar 3: only show numbers that are real).
#[derive(Debug, Clone)]
pub struct RiskFactor {
    pub label: &'static str,
    pub points: i32,
}

#[derive(Debug, Clone, Default)]
pub struct FailureRisk {
    pub total: i32,
    pub at_risk: bool,
    pub factors: Vec<RiskFactor>,
}

/// The §5.5 failure-risk formula. Cultural drift and unity threaten every
/// legacy; the legacy-specific counters only threaten the legacy whose
/// failure condition they belong to.
pub fn failure_risk(sim: &SimState, config: &GameConfig) -> FailureRisk {
    let fr = &config.failure_risk;
    let mut risk = FailureRisk::default();
    let mut add = |label, points| risk.factors.push(RiskFactor { label, points });

    if sim.population.cultural_drift > fr.drift_threshold {
        add("Cultural drift runs high", fr.drift_points);
    }
    if sim.population.unity < fr.unity_threshold {
        add("Unity has frayed", fr.unity_points);
    }
    match sim.legacy.legacy_id.as_str() {
        "preservers" => {
            if sim.legacy.tradition_points < fr.tradition_threshold {
                add("Tradition nears extinction", fr.tradition_points);
            }
        }
        "adaptors" => {
            if sim.legacy.body_horror_events >= fr.body_horror_threshold {
                add("The modifications have a cost", fr.body_horror_points);
            }
            if sim.legacy.existential_dread > fr.dread_threshold {
                add("Existential dread spreads", fr.dread_points);
            }
        }
        "wanderers" if sim.legacy.piracy_reputation > fr.piracy_threshold => {
            add("Piracy invites reprisal", fr.piracy_points);
        }
        _ => {}
    }

    risk.total = risk.factors.iter().map(|f| f.points).sum();
    risk.at_risk = risk.total > fr.at_risk_threshold;
    risk
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn failure_risk_matches_gdd_formula() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );

        let calm = failure_risk(&sim, &data.config);
        assert_eq!(calm.total, 0);
        assert!(!calm.at_risk);

        sim.population.cultural_drift = 0.8; // +30
        sim.population.unity = 0.2; // +25
        sim.legacy.tradition_points = 10; // +35
        let dire = failure_risk(&sim, &data.config);
        assert_eq!(dire.total, 90);
        assert!(dire.at_risk);
        assert_eq!(dire.factors.len(), 3);
    }

    #[test]
    fn legacy_specific_counters_only_threaten_their_own_legacy() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            2,
            &crate::state::sim::founding_faction_ids(&data),
        );
        // Adaptors' counters must not add risk to a Wanderer campaign.
        sim.legacy.body_horror_events = 10;
        sim.legacy.existential_dread = 1.0;
        assert_eq!(failure_risk(&sim, &data.config).total, 0);

        sim.legacy.piracy_reputation = 0.9;
        let risky = failure_risk(&sim, &data.config);
        assert_eq!(risky.total, data.config.failure_risk.piracy_points);
    }

    fn plain_option(chance: f32) -> DilemmaOption {
        DilemmaOption {
            id: "opt".into(),
            label: "opt".into(),
            success_chance: chance,
            success: DilemmaEffect::default(),
            failure: DilemmaEffect::default(),
            dominant_faction: String::new(),
            dominant_faction_odds: 0.0,
        }
    }

    #[test]
    fn combat_lifts_wanderer_dilemma_odds_only() {
        let data = GameData::load().unwrap();
        // A Wanderer ship with a weapon installed beats its base odds.
        let mut wanderer = SimState::new_campaign(
            &data,
            "wanderers",
            4,
            &crate::state::sim::founding_faction_ids(&data),
        );
        wanderer.ship.weapon = Some("mass_driver".to_owned()); // combat 5
        let lifted = dilemma_odds(&wanderer, &data, &plain_option(0.65));
        assert!(lifted > 0.65, "combat should raise Wanderer odds: {lifted}");
        assert!(lifted <= data.config.ship.dilemma_odds_cap);

        // The same weapon does nothing for another legacy's dilemmas.
        let mut preserver = SimState::new_campaign(
            &data,
            "preservers",
            4,
            &crate::state::sim::founding_faction_ids(&data),
        );
        preserver.ship.weapon = Some("mass_driver".to_owned());
        assert_eq!(dilemma_odds(&preserver, &data, &plain_option(0.65)), 0.65);
    }

    #[test]
    fn the_dominant_faction_backs_or_hinders_a_dilemma_gamble() {
        // Content-depth factions round 10: who runs the ship shifts the odds of a
        // defining gamble. A backed option reads higher only while its faction is
        // dominant; a hindered one reads lower.
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "adaptors",
            4,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let mut backed = plain_option(0.6);
        backed.dominant_faction = "ascension_circle".into();
        backed.dominant_faction_odds = 0.15;

        // Make the Ascension the sole (hence dominant) people: odds lift.
        sim.factions = vec![crate::state::sim::factions::FactionState {
            faction_id: "ascension_circle".into(),
            members: 1000,
            status: crate::state::sim::factions::FactionStatus::Aboard,
            approval: 0.5,
            mood_band: 0,
        }];
        let with = dilemma_odds(&sim, &data, &backed);
        assert!(with > 0.6, "the augmented back the augmentation: {with}");

        // A different dominant people: no lift.
        sim.factions[0].faction_id = "first_flame".into();
        assert_eq!(
            dilemma_odds(&sim, &data, &backed),
            0.6,
            "another people neither backs nor hinders it"
        );
    }

    #[test]
    fn resolve_dilemma_applies_a_branch_and_updates_counters() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.pending_dilemma = Some(PendingDilemma {
            dilemma_id: "archive_purge".to_owned(),
            rolled_month_clock: sim.month_clock,
        });

        let tradition_before = sim.legacy.tradition_points;
        let log_len = sim.log.len();
        let text = resolve_dilemma(&mut sim, &data, 0).expect("dilemma must resolve");
        assert!(sim.pending_dilemma.is_none());
        assert_eq!(sim.log.len(), log_len + 1);
        assert!(!text.is_empty());
        // Option 0 ("protect the archive"): success grants +10 tradition,
        // failure costs food/morale but leaves tradition alone.
        let succeeded = sim.legacy.tradition_points != tradition_before;
        if succeeded {
            assert_eq!(sim.legacy.tradition_points, tradition_before + 10);
        }
    }

    #[test]
    fn dilemma_resolution_is_deterministic_per_seed() {
        let data = GameData::load().unwrap();
        let mut runs = Vec::new();
        for _ in 0..2 {
            let mut sim = SimState::new_campaign(
                &data,
                "adaptors",
                99,
                &crate::state::sim::founding_faction_ids(&data),
            );
            sim.pending_dilemma = Some(PendingDilemma {
                dilemma_id: "gene_clinic".to_owned(),
                rolled_month_clock: 0,
            });
            resolve_dilemma(&mut sim, &data, 0);
            runs.push((
                sim.legacy.body_horror_events,
                sim.population.adaptation.to_bits(),
            ));
        }
        assert_eq!(runs[0], runs[1]);
    }
}
