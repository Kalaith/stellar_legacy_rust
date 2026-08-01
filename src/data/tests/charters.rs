//! Charters: every one is a generational voyage, its phases sum to its
//! duration, and every family, deed and gate it names is real.

use super::*;

/// W1-rescale and W2: >= 300 years, phases summing exactly to it.
#[test]
fn every_charter_is_a_generational_voyage_with_authored_phases() {
    let data = GameData::load().unwrap();
    // Charter tiering (PLAN M4.8): some charters gate behind renown, some
    // are available from the founding.
    assert!(
        data.contracts.iter().any(|(_, c)| c.min_renown > 0),
        "some charters should unlock with renown"
    );
    assert!(
        data.contracts.iter().any(|(_, c)| c.min_renown == 0),
        "some charters should be available from the founding"
    );
    // W1-rescale: every charter is now a generational voyage (>= 300 yr).
    // W2: authored phases sum exactly to the duration, only Travel/Operation/
    // Return kinds, at least one Operation segment, and a real objective.
    use contracts::ContractPhase;
    for (id, c) in data.contracts.iter() {
        assert!(
            c.target_duration_years >= 300,
            "charter '{id}' must be a generational voyage (>= 300 yr), is {}",
            c.target_duration_years
        );
        let phase_years: u32 = c.phases.iter().map(|p| p.years).sum();
        assert_eq!(
            phase_years, c.target_duration_years,
            "charter '{id}' phase years {phase_years} must sum to its duration {}",
            c.target_duration_years
        );
        for phase in &c.phases {
            assert!(
                matches!(
                    phase.kind,
                    ContractPhase::Travel | ContractPhase::Operation | ContractPhase::Return
                ),
                "charter '{id}' has an invalid authored phase kind {:?}",
                phase.kind
            );
        }
        assert!(
            c.phases.iter().any(|p| p.kind == ContractPhase::Operation),
            "charter '{id}' must have at least one Operation segment"
        );
        assert!(
            c.objective_target > 0.0,
            "charter '{id}' must have a positive objective target"
        );
    }
}

/// A charter that names a family, deed or trait that does not exist
/// would silently never fire the content it promises.
#[test]
fn every_charter_names_real_families_deeds_and_gates() {
    let data = GameData::load().unwrap();
    let sk = &data.config.campaign_skeleton;
    let families = authored_families(&data);
    // Content-depth charters round 7: a charter's beat-pool bias must name
    // real families, or a biased beat could land on an empty pool. At least
    // one charter must carry a bias, so the mechanic stays exercised.
    assert!(
        data.contracts
            .iter()
            .any(|(_, c)| !c.beat_families.is_empty()),
        "some charter should bias its seeded skeleton via beat_families"
    );
    for (id, c) in data.contracts.iter() {
        for fam in &c.beat_families {
            assert!(
                families.contains(fam),
                "charter '{id}' beat_families '{fam}' has no events"
            );
        }
        // Content-depth charters round 9: a scripted timed beat must name a
        // real, scheduled_only event, and the beats must ascend by year so
        // they fire in order.
        for beat in &c.scheduled_beats {
            let target = data.events.get(&beat.template_id);
            assert!(
                target.is_some_and(|e| e.scheduled_only),
                "charter '{id}' scheduled beat '{}' must be a scheduled_only event",
                beat.template_id
            );
        }
        assert!(
            c.scheduled_beats
                .windows(2)
                .all(|w| w[0].at_year <= w[1].at_year),
            "charter '{id}' scheduled_beats must ascend by at_year"
        );
        // Content-depth charters round 11: route hazard is a sane weight bump.
        assert!(
            (0.0..=1.0).contains(&c.hazard),
            "charter '{id}' hazard {} out of range [0, 1]",
            c.hazard
        );
        // Content-depth charters round 12: an in-world availability gate must
        // name real founding peoples, or the writ could never be offered.
        for fid in &c.requires_faction_aboard {
            assert!(
                data.factions.get(fid).is_some(),
                "charter '{id}' requires unknown faction '{fid}' aboard"
            );
        }
        // Content-depth charters round 19: a completion goodwill reward must name
        // a real people, or the goodwill would land nowhere.
        for d in &c.completion_reward.faction_approval_deltas {
            assert!(
                data.factions.get(&d.id).is_some(),
                "charter '{id}' completion_reward names unknown faction '{}'",
                d.id
            );
        }
        // Content-depth charters round 20: a completion component reward must name
        // a real ship component, or the salvage hold gains a phantom.
        if let Some(comp) = &c.completion_reward.grant_component {
            assert!(
                data.ship_components.find_any(comp).is_some(),
                "charter '{id}' completion_reward grant_component '{comp}' is not a real component"
            );
        }
        // Content-depth charters round 13: a route toll must be a gentle,
        // survivable headwind — a per-year crew drain that could empty a
        // generational voyage is a bug, not a hazard.
        assert!(
            c.annual_toll.population.count.abs() <= 3,
            "charter '{id}' annual_toll drains {} crew/yr — too steep for a voyage",
            c.annual_toll.population.count
        );
        // Content-depth subsystems round 14: the module a mission leans on must
        // be a real subsystem, or its condition could never scale the work.
        assert!(
            c.objective_subsystem.is_empty()
                || data.subsystems.get(&c.objective_subsystem).is_some(),
            "charter '{id}' objective_subsystem names unknown module '{}'",
            c.objective_subsystem
        );
        // Content-depth charters round 21: a mission's combat scaling is a
        // positive accelerator (firepower quickens contested work, never slows
        // it) and gentle — an over-steep value would make the drydock's guns the
        // only thing that matters. Bounded like the speed lever's reach.
        assert!(
            (0.0..=0.2).contains(&c.objective_combat_scaling),
            "charter '{id}' objective_combat_scaling {} out of range [0, 0.2]",
            c.objective_combat_scaling
        );
        // Content-depth charters round 24: cargo scaling is a small per-unit rate
        // (cargo counts in the hundreds, not the single digits combat does), so its
        // ceiling is far lower — a big hold helps a haul, it does not dominate it.
        assert!(
            (0.0..=0.01).contains(&c.objective_cargo_scaling),
            "charter '{id}' objective_cargo_scaling {} out of range [0, 0.01]",
            c.objective_cargo_scaling
        );
        // Content-depth charters round 26: loadout gates are non-negative minimums.
        assert!(
            c.min_combat >= 0 && c.min_cargo >= 0 && c.min_speed >= 0,
            "charter '{id}' has a negative loadout requirement"
        );
        // Content-depth charters round 29: a reputation-scaled reward must name a positive
        // scale (a trait with a zero scale is a dead field), and the scale is gentle so a
        // name is worth a premium but never multiplies or erases the pay outright.
        if !c.reward_reputation_trait.is_empty() {
            assert!(
                (0.0..=1.0).contains(&c.reward_reputation_scale) && c.reward_reputation_scale > 0.0,
                "charter '{id}' names a reward reputation trait but its scale {} is not in (0, 1]",
                c.reward_reputation_scale
            );
        }
        // Content-depth charters round 23: a preserve charter must actually erode
        // (a positive, gentle yearly attrition), or "keep the cargo" is a free win.
        if c.preserve_objective {
            assert!(
                c.preserve_attrition_per_year > 0.0 && c.preserve_attrition_per_year <= 0.01,
                "charter '{id}' preserve_attrition_per_year {} must be a gentle positive rate",
                c.preserve_attrition_per_year
            );
        }
        // Content-depth charters round 15: a completion reward's subsystem boons
        // must name real modules, or the legacy could never land.
        for delta in &c.completion_reward.subsystem_deltas {
            assert!(
                data.subsystems.get(&delta.id).is_some(),
                "charter '{id}' completion_reward names unknown module '{}'",
                delta.id
            );
        }
    }
    // Content-depth charters round 13: at least one charter should carry a
    // standing route toll, so the mechanic is exercised.
    assert!(
        data.contracts.iter().any(|(_, c)| !c.annual_toll.is_none()),
        "some charter should exact a per-year route toll"
    );
    // Content-depth charters round 21: at least one charter should reward
    // firepower (a contested writ worked faster by an armed ship), so the
    // charter↔combat coupling is actually exercised.
    assert!(
        data.contracts
            .iter()
            .any(|(_, c)| c.objective_combat_scaling > 0.0),
        "some charter should let combat quicken its objective"
    );
    // Content-depth charters round 14: a charter's deed gates must name a
    // consequence *something* produces — an event outcome or another charter's
    // completion — or the writ (or its bar) could never resolve (typo guard).
    let charter_produced: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .flat_map(|o| o.long_term_consequences.iter())
        .chain(
            data.contracts
                .iter()
                .filter(|(_, c)| !c.completion_consequence.is_empty())
                .map(|(_, c)| &c.completion_consequence),
        )
        // Content-depth charters round 30: a *failed* charter's deed-mark is a producer too.
        .chain(
            data.contracts
                .iter()
                .filter(|(_, c)| !c.failure_consequence.is_empty())
                .map(|(_, c)| &c.failure_consequence),
        )
        .collect();
    for (id, c) in data.contracts.iter() {
        for tag in c
            .requires_consequence
            .iter()
            .chain(c.forbidden_consequence.iter())
        {
            assert!(
                charter_produced.contains(tag),
                "charter '{id}' gates on consequence '{tag}' nothing records"
            );
        }
    }
    // Content-depth charters round 12: at least one charter should key on an
    // in-world gate, so the mechanic is exercised.
    assert!(
        data.contracts
            .iter()
            .any(|(_, c)| !c.requires_faction_aboard.is_empty()),
        "some charter should gate on a people being aboard"
    );
    // Content-depth round 5: the dead-air backstop needs a pool to draw from
    // when it is switched on, or a forced beat has nothing to force.
    if sk.dead_air_years > 0 {
        assert!(
            !sk.dead_air_pool.is_empty(),
            "dead_air_years is set but dead_air_pool is empty"
        );
    }
}

/// Charter-family scoring (content-depth charters round 35): every charter's
/// scorecard must sum to 1.0 and must carry exactly one *family* metric — the
/// signature grade its objective family is judged on beyond the four universal
/// ones. Without it the whole board is four routes through the same scorecard.
#[test]
fn every_charter_is_graded_on_something_its_family_alone_is_graded_on() {
    use crate::data::contracts::{ContractObjective, MetricKind};
    let data = GameData::load().unwrap();
    // The signature grade each objective family answers to.
    let expected = |objective: ContractObjective| match objective {
        ContractObjective::Exploration => MetricKind::KnowledgeRetained,
        ContractObjective::Mining | ContractObjective::Salvage => MetricKind::ShipCondition,
        ContractObjective::Rescue | ContractObjective::Diplomacy => MetricKind::Reputation,
        ContractObjective::Colonization => MetricKind::FoundersCovenant,
    };
    // The traits some outcome can actually move, so a reputation grade is never
    // pinned to a name the ship has no way of earning.
    let rep_produced: std::collections::HashSet<&String> = data
        .events
        .iter()
        .flat_map(|(_, e)| e.outcomes.iter())
        .flat_map(|o| o.reputation_deltas.iter().map(|r| &r.id))
        .collect();

    for (id, c) in data.contracts.iter() {
        let sum: f32 = c.success_metrics.iter().map(|m| m.weight).sum();
        assert!(
            (sum - 1.0).abs() < 1e-4,
            "charter '{id}' scorecard weights sum to {sum}, not 1.0"
        );
        let want = expected(c.objective);
        let family: Vec<_> = c
            .success_metrics
            .iter()
            .filter(|m| {
                !matches!(
                    m.kind,
                    MetricKind::PopulationSurvival
                        | MetricKind::MissionCompletion
                        | MetricKind::ResourceEfficiency
                        | MetricKind::SocialCohesion
                )
            })
            .collect();
        assert_eq!(
            family.len(),
            1,
            "charter '{id}' should carry exactly one family metric, has {}",
            family.len()
        );
        let metric = family[0];
        assert_eq!(
            metric.kind, want,
            "charter '{id}' is a {:?} writ and must be graded on {want:?}",
            c.objective
        );
        assert!(
            metric.weight > 0.0 && metric.target > 0.0,
            "charter '{id}' family metric '{}' must actually count",
            metric.id
        );
        if metric.kind == MetricKind::Reputation {
            assert!(
                rep_produced.contains(&metric.trait_id),
                "charter '{id}' grades reputation '{}' no outcome nudges",
                metric.trait_id
            );
        }
    }
}
