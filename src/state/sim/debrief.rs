//! The record a voyage leaves behind: the beats worth remembering, captured as
//! they happen, and the sealed homecoming report the debrief screen reads.
//!
//! Both exist because the running state cannot answer "what happened on this
//! voyage" once it is over. The ship's log is trimmed to `log_limit` (200
//! lines), so a 300-year crossing has forgotten its own departure by the time
//! it docks; `sim.contract` is cleared the moment a charter concludes, taking
//! the metrics and milestones with it. So the notable beats accrue onto the
//! active contract while it flies, and completion seals a snapshot that
//! survives the contract's own end (and a save/quit in the middle of reading
//! it).

use crate::data::ResourceDelta;
use crate::state::sim::dynasty::Reign;
use serde::{Deserialize, Serialize};

/// What kind of beat a highlight records — the debrief tags each line with
/// this, and the capture cap keeps the rarer kinds in preference to the common
/// one.
///
/// Captaincies are deliberately absent: the debrief's chain-of-command column
/// names every captain a charter passed through in more detail than a log line
/// could, and a full-length charter changes captains often enough that logging
/// each one crowded out the council's decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightKind {
    /// A charter milestone came up on the timeline.
    Milestone,
    /// The council was asked, and answered — the player's own decision.
    Decision,
    /// The charter's phase turned: outbound, on station, homeward.
    Phase,
}

impl HighlightKind {
    /// Short tag shown at the head of the line in the debrief's log column.
    pub fn tag(self) -> &'static str {
        match self {
            HighlightKind::Milestone => "MARK",
            HighlightKind::Decision => "COUNCIL",
            HighlightKind::Phase => "LEG",
        }
    }
}

/// One remembered beat of a voyage, stamped with the month it happened in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoyageHighlight {
    pub year: u32,
    pub month: u32,
    pub kind: HighlightKind,
    pub text: String,
}

/// One success metric as it stood at the homecoming, with the target it was
/// scored against — the debrief shows the arithmetic behind the final band
/// rather than only its verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebriefMetric {
    pub name: String,
    pub current: f32,
    pub target: f32,
    pub weight: f32,
}

impl DebriefMetric {
    /// The metric's own contribution to the score, on the same
    /// `min(1, current/target) * weight` terms `contract::score_success` uses.
    pub fn contribution(&self) -> f32 {
        if self.target <= 0.0 {
            return self.weight;
        }
        (self.current / self.target).clamp(0.0, 1.0) * self.weight
    }
}

/// One charter milestone and whether the voyage actually reached it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebriefMilestone {
    pub name: String,
    pub reached: bool,
}

/// The sealed homecoming report. Written when a charter concludes and cleared
/// when the player dismisses the debrief screen; serialized so quitting
/// mid-read and loading back returns to it rather than silently swallowing the
/// voyage's only summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoyageDebrief {
    pub contract_name: String,
    pub objective: String,
    /// `SuccessLevel::label()` for the band the score fell in.
    pub outcome: String,
    pub score: f32,
    pub began_year: u32,
    pub ended_year: u32,
    /// Full years the charter ran — its own clock, not the campaign's.
    pub duration_years: u32,
    /// Dynasty generations that turned over between launch and homecoming.
    pub generations: u32,
    /// What the charter actually paid, after the objective proration and the
    /// ship's reputation multiplier — the number the player can check against
    /// the writ they accepted.
    pub payout: ResourceDelta,
    pub metrics: Vec<DebriefMetric>,
    pub milestones: Vec<DebriefMilestone>,
    pub highlights: Vec<VoyageHighlight>,
    /// Every captain who held the chair between launch and homecoming, oldest
    /// first. Empty only on a save written before the reign roster existed.
    pub commanders: Vec<Reign>,
    pub population_start: u32,
    pub population_end: u32,
    /// The authored homecoming prose for this outcome band, if the pool had a
    /// line for it.
    pub homecoming_line: Option<String>,
    /// The lasting capability a completed charter left the ship, if any.
    pub legacy_line: Option<String>,
}

impl Default for VoyageDebrief {
    fn default() -> Self {
        Self {
            contract_name: String::new(),
            objective: String::new(),
            outcome: String::new(),
            score: 0.0,
            began_year: 0,
            ended_year: 0,
            duration_years: 0,
            generations: 0,
            payout: ResourceDelta::default(),
            metrics: Vec::new(),
            milestones: Vec::new(),
            highlights: Vec::new(),
            commanders: Vec::new(),
            population_start: 0,
            population_end: 0,
            homecoming_line: None,
            legacy_line: None,
        }
    }
}

impl VoyageDebrief {
    /// Milestones reached over milestones offered — the headline the timeline
    /// column leads with.
    pub fn milestones_reached(&self) -> (usize, usize) {
        (
            self.milestones.iter().filter(|m| m.reached).count(),
            self.milestones.len(),
        )
    }

    /// Net head count across the voyage: positive if the ship came home with
    /// more people aboard than it left with.
    pub fn population_change(&self) -> i64 {
        self.population_end as i64 - self.population_start as i64
    }

    /// True when the charter paid nothing at all — a total default, which the
    /// payout column states outright rather than showing five zeroes.
    pub fn unpaid(&self) -> bool {
        let p = &self.payout;
        p.credits == 0 && p.energy == 0 && p.minerals == 0 && p.food == 0 && p.influence == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_metric_contributes_its_weight_only_when_it_meets_target() {
        let metric = |current, target, weight| DebriefMetric {
            name: "m".to_owned(),
            current,
            target,
            weight,
        };
        // Met exactly: the full weight.
        assert!((metric(0.9, 0.9, 0.3).contribution() - 0.3).abs() < 1e-6);
        // Half way there: half the weight.
        assert!((metric(0.45, 0.9, 0.3).contribution() - 0.15).abs() < 1e-6);
        // Overshooting does not earn more than the weight — the same clamp the
        // scorer applies, so the debrief's arithmetic matches the band shown.
        assert!((metric(2.0, 0.9, 0.3).contribution() - 0.3).abs() < 1e-6);
        // A zero target cannot be divided by; it counts as met.
        assert!((metric(0.0, 0.0, 0.25).contribution() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn the_report_summarizes_its_own_tallies() {
        let mut debrief = VoyageDebrief {
            milestones: vec![
                DebriefMilestone {
                    name: "Departure burn complete".to_owned(),
                    reached: true,
                },
                DebriefMilestone {
                    name: "Halfway beacon passed".to_owned(),
                    reached: true,
                },
                DebriefMilestone {
                    name: "Colony charter signed".to_owned(),
                    reached: false,
                },
            ],
            population_start: 900,
            population_end: 845,
            ..Default::default()
        };
        assert_eq!(debrief.milestones_reached(), (2, 3));
        assert_eq!(debrief.population_change(), -55);
        assert!(debrief.unpaid(), "a default report has paid nothing");
        debrief.payout.credits = 12_000;
        assert!(!debrief.unpaid());
    }

    #[test]
    fn an_older_report_shape_loads_with_defaults() {
        // The record is `serde(default)` throughout so a save written by an
        // earlier build reads back as a sparse report rather than a hard error
        // that would cost the player the campaign.
        let debrief: VoyageDebrief =
            serde_json::from_str(r#"{"contract_name":"The Long Tow","score":0.62}"#).unwrap();
        assert_eq!(debrief.contract_name, "The Long Tow");
        assert!((debrief.score - 0.62).abs() < 1e-6);
        assert!(debrief.commanders.is_empty());
        assert_eq!(debrief.milestones_reached(), (0, 0));
    }
}
