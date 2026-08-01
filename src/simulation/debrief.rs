//! Remembering a voyage, and sealing the report it comes home with.
//!
//! Two halves of one job. `remember` is called from the tick, the succession
//! path, and the event resolver as beats happen — the running record, kept on
//! the active contract. `seal` is called once, at the moment a charter
//! concludes, and snapshots everything the homecoming screen needs before
//! `sim.contract` is cleared out from under it.

use crate::data::{GameData, ResourceDelta};
use crate::simulation::contract::SuccessLevel;
use crate::state::sim::debrief::{DebriefMetric, DebriefMilestone, HighlightKind, VoyageDebrief};
use crate::state::sim::SimState;

/// Remember a beat of the voyage under way. A no-op in port — there is no
/// voyage to recap — so callers need not check for a contract themselves.
pub fn remember(sim: &mut SimState, data: &GameData, kind: HighlightKind, text: impl Into<String>) {
    let (year, month) = (sim.year(), sim.month());
    let limit = data.config.voyage_highlight_limit;
    if let Some(contract) = sim.contract.as_mut() {
        contract.push_highlight(year, month, kind, text, limit);
    }
}

/// Seal the homecoming report for the charter that just concluded. Must be
/// called while `sim.contract` is still set — it reads the metrics, milestones,
/// and remembered beats straight off it.
///
/// `payout` is what the charter actually paid after proration and reputation,
/// not the writ's headline reward; `homecoming_line` and `legacy_line` are the
/// authored prose already pushed to the log, repeated here so the debrief can
/// lead with them instead of asking the player to go find them.
pub fn seal(
    sim: &SimState,
    score: f32,
    level: SuccessLevel,
    payout: ResourceDelta,
    homecoming_line: Option<String>,
    legacy_line: Option<String>,
) -> Option<VoyageDebrief> {
    let contract = sim.contract.as_ref()?;
    let ended_year = sim.year();
    // The charter's own clock, not the campaign's: a fuel stall stretches the
    // wall-clock years without advancing the contract, and the writ was flown
    // against the latter.
    let duration_years = contract.months_elapsed / 12;
    let began_year = contract.began_year;

    let commanders = sim
        .dynasty
        .reigns
        .iter()
        .filter(|reign| reign.overlaps(began_year, ended_year))
        .cloned()
        .collect();

    Some(VoyageDebrief {
        contract_name: contract.name.clone(),
        objective: contract.objective.label().to_owned(),
        outcome: level.label().to_owned(),
        score,
        began_year,
        ended_year,
        duration_years,
        generations: sim
            .dynasty
            .generation
            .saturating_sub(contract.began_generation),
        payout,
        metrics: contract
            .metrics
            .iter()
            .map(|m| DebriefMetric {
                name: m.name.clone(),
                current: m.current,
                target: m.target,
                weight: m.weight,
            })
            .collect(),
        milestones: contract
            .milestones
            .iter()
            .map(|m| DebriefMilestone {
                name: m.name.clone(),
                reached: m.reached,
            })
            .collect(),
        highlights: contract.highlights.clone(),
        commanders,
        population_start: contract.starting_population,
        population_end: sim.population.count,
        homecoming_line,
        legacy_line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::contract;
    use crate::state::sim::founding_faction_ids;

    /// A campaign with a charter under way, ready to accrue beats.
    fn launched() -> (GameData, SimState) {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 7, &founding_faction_ids(&data));
        let template = data.contracts.get("the_long_tow").unwrap().clone();
        sim.contract = Some(contract::start_contract(&template, &sim));
        (data, sim)
    }

    #[test]
    fn beats_are_remembered_under_way_and_ignored_in_port() {
        let (data, mut sim) = launched();
        remember(&mut sim, &data, HighlightKind::Milestone, "Halfway beacon");
        assert_eq!(sim.contract.as_ref().unwrap().highlights.len(), 1);
        let beat = &sim.contract.as_ref().unwrap().highlights[0];
        assert_eq!(beat.text, "Halfway beacon");
        assert_eq!(beat.kind, HighlightKind::Milestone);
        assert_eq!(beat.year, sim.year());

        // In port there is no voyage to recap; the call must not panic or
        // strand the beat anywhere.
        sim.contract = None;
        remember(&mut sim, &data, HighlightKind::Decision, "ignored");
    }

    #[test]
    fn the_remembered_beats_stay_within_their_cap() {
        // A 450-year charter must not grow the save without bound. Past the cap
        // the oldest decisions fall away and the recent voyage stays legible.
        let (data, mut sim) = launched();
        let limit = data.config.voyage_highlight_limit;
        assert!(limit > 0, "the cap must be configured");
        for i in 0..(limit + 15) {
            remember(
                &mut sim,
                &data,
                HighlightKind::Decision,
                format!("beat {i}"),
            );
        }
        let highlights = &sim.contract.as_ref().unwrap().highlights;
        assert_eq!(highlights.len(), limit, "the cap holds");
        assert_eq!(
            highlights.last().unwrap().text,
            format!("beat {}", limit + 14),
            "the newest beat survives"
        );
        assert_eq!(
            highlights.first().unwrap().text,
            "beat 15",
            "the oldest beats fell away, in order"
        );
    }

    #[test]
    fn a_full_length_charter_keeps_its_skeleton_and_gives_up_its_decisions() {
        // The failure this guards against: a 450-year charter overruns the cap
        // on council decisions alone, and a naive drop-the-oldest leaves the
        // debrief showing only the final decade — no departure, no halfway
        // beacon, none of the early captaincies. The structural beats are few
        // and spread across the whole voyage, so they are kept.
        let (data, mut sim) = launched();
        let limit = data.config.voyage_highlight_limit;

        // The voyage's skeleton, laid down early.
        remember(&mut sim, &data, HighlightKind::Phase, "Travel");
        remember(&mut sim, &data, HighlightKind::Milestone, "Departure burn");
        remember(&mut sim, &data, HighlightKind::Milestone, "Halfway beacon");
        // …then far more decisions than the cap can hold.
        for i in 0..(limit * 3) {
            remember(
                &mut sim,
                &data,
                HighlightKind::Decision,
                format!("choice {i}"),
            );
        }

        let highlights = &sim.contract.as_ref().unwrap().highlights;
        assert_eq!(highlights.len(), limit, "the cap still holds");
        let structural: Vec<&str> = highlights
            .iter()
            .filter(|h| h.kind != HighlightKind::Decision)
            .map(|h| h.text.as_str())
            .collect();
        assert_eq!(
            structural,
            vec!["Travel", "Departure burn", "Halfway beacon"],
            "every leg and mark survived the flood of decisions"
        );
        assert_eq!(
            highlights.last().unwrap().text,
            format!("choice {}", limit * 3 - 1),
            "and the most recent decision is still the last line"
        );
    }

    #[test]
    fn a_voyage_of_nothing_but_structure_still_respects_the_cap() {
        // The degenerate case: with no decisions to give up, the oldest beat of
        // any kind gives way rather than the list growing forever.
        let (data, mut sim) = launched();
        let limit = data.config.voyage_highlight_limit;
        for i in 0..(limit + 5) {
            remember(
                &mut sim,
                &data,
                HighlightKind::Milestone,
                format!("mark {i}"),
            );
        }
        let highlights = &sim.contract.as_ref().unwrap().highlights;
        assert_eq!(highlights.len(), limit);
        assert_eq!(highlights.first().unwrap().text, "mark 5");
    }

    #[test]
    fn sealing_snapshots_what_the_cleared_contract_would_take_with_it() {
        let (data, mut sim) = launched();
        remember(&mut sim, &data, HighlightKind::Milestone, "Tow secured");
        // Reach one milestone so the tally has something to count.
        if let Some(contract) = sim.contract.as_mut() {
            contract.months_elapsed = 60;
            if let Some(first) = contract.milestones.first_mut() {
                first.reached = true;
            }
        }
        let payout = ResourceDelta {
            credits: 9_000,
            influence: 40,
            ..Default::default()
        };
        let report = seal(
            &sim,
            0.74,
            SuccessLevel::Partial,
            payout,
            Some("The ship came home lighter than she left.".to_owned()),
            None,
        )
        .expect("a contract is under way");

        assert_eq!(report.outcome, "Partial");
        assert!((report.score - 0.74).abs() < 1e-6);
        assert_eq!(report.duration_years, 5, "the charter's clock, in years");
        assert_eq!(report.payout.credits, 9_000);
        assert!(!report.unpaid());
        assert!(!report.metrics.is_empty(), "the scorecard is carried over");
        assert_eq!(report.milestones_reached().0, 1);
        assert!(report.milestones_reached().1 >= 1);
        assert_eq!(report.highlights.len(), 1);
        assert_eq!(report.highlights[0].text, "Tow secured");
        assert!(
            report.homecoming_line.is_some(),
            "the authored prose rides along"
        );
        assert_eq!(report.population_start, report.population_end);

        // The report outlives the contract that produced it — the whole point.
        sim.contract = None;
        assert!(report.contract_name.contains("The Long Tow"));
        assert!(seal(&sim, 0.0, SuccessLevel::Failure, payout, None, None).is_none());
    }

    #[test]
    fn the_report_names_only_the_captains_who_held_the_chair_that_voyage() {
        let (_data, mut sim) = launched();
        // Launch the charter at year 50, so there is room on the campaign clock
        // for a captaincy that ended *before* it — the case that must not be
        // credited to this voyage.
        let began = 50;
        if let Some(contract) = sim.contract.as_mut() {
            contract.began_year = began;
        }
        sim.dynasty.reigns.clear();
        sim.dynasty.reigns.push(crate::state::sim::Reign {
            name: "Old Guard".to_owned(),
            began_year: 10,
            ended_year: Some(49),
            generation: 1,
            leadership: 60,
            trait_name: String::new(),
        });
        sim.dynasty.begin_reign(began);
        // A handoff mid-voyage: both the outgoing and incoming captain count.
        sim.dynasty.end_reign(began + 20);
        sim.dynasty.begin_reign(began + 20);
        // …and the ship docks at year 90, twenty years after that handoff.
        sim.month_clock = 90 * 12;

        let report = seal(
            &sim,
            0.5,
            SuccessLevel::Partial,
            ResourceDelta::default(),
            None,
            None,
        )
        .expect("a contract is under way");
        assert_eq!(
            report.commanders.len(),
            2,
            "the two captaincies that overlapped the voyage, not the one before it"
        );
        assert!(
            !report.commanders.iter().any(|c| c.name == "Old Guard"),
            "a reign that ended before the launch is not credited to this voyage"
        );
    }
}
