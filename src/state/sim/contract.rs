//! Active-contract runtime state: the mission a ship is currently flying — its
//! authored phase timeline (W2), quantified objective (W2), success metrics and
//! milestones, and the seeded campaign beats (W6). Split out of `sim.rs` to keep
//! that file under the size limit.

use crate::data::ResourceDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricState {
    pub id: String,
    pub kind: crate::data::contracts::MetricKind,
    pub name: String,
    pub weight: f32,
    pub target: f32,
    pub current: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneState {
    pub id: String,
    pub name: String,
    pub progress_threshold: f32,
    pub reached: bool,
    /// One-time resources granted when first reached (PLAN item 3).
    #[serde(default)]
    pub reward: ResourceDelta,
}

/// One scheduled major beat of a mission's campaign skeleton (W6): an absolute
/// month it should fire and the event family it draws from. Laid out
/// deterministically at LAUNCH so the same seed replays the same campaign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignBeat {
    pub month_clock: u32,
    pub family: String,
    pub fired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveContract {
    pub template_id: String,
    pub name: String,
    pub objective: crate::data::contracts::ContractObjective,
    pub target_duration_years: u32,
    /// Contract time elapsed, month-precise (W2/W3). Drives the phase timeline,
    /// the progress bar, and completion.
    pub months_elapsed: u32,
    /// Current phase, set from the authored segments — never derived from a
    /// fraction (W2).
    pub phase: crate::data::contracts::ContractPhase,
    /// The charter's authored travel → operation → return segments (W2), copied
    /// at start so the active contract carries its own timeline.
    pub phases: Vec<crate::data::contracts::PhaseDef>,
    /// Index into `phases` for the current segment.
    pub phase_index: usize,
    pub metrics: Vec<MetricState>,
    pub milestones: Vec<MilestoneState>,
    /// Population when the contract began, for the survival metric.
    pub starting_population: u32,
    /// Quantified objective amount for full pay (W2), copied from the charter.
    pub objective_target: f32,
    /// Human unit for the objective counter.
    pub objective_unit: String,
    /// Objective amount reached so far — accrues only during Operation (W2).
    pub objective_progress: f32,
    /// Seeded campaign beats (W6), generated at LAUNCH; the monthly loop fires
    /// each when its month arrives.
    #[serde(default)]
    pub beats: Vec<CampaignBeat>,
}

impl ActiveContract {
    /// Total contract length in months.
    pub fn total_months(&self) -> u32 {
        self.target_duration_years * 12
    }

    /// Timeline position as a 0-1 fraction (milestones + the UI bar).
    pub fn progress(&self) -> f32 {
        let total = self.total_months();
        if total == 0 {
            1.0
        } else {
            (self.months_elapsed as f32 / total as f32).min(1.0)
        }
    }

    /// Fraction of the quantified objective reached — the pay multiplier (W2).
    /// A target of 0 counts as fully met.
    pub fn objective_fraction(&self) -> f32 {
        if self.objective_target <= 0.0 {
            1.0
        } else {
            (self.objective_progress / self.objective_target).clamp(0.0, 1.0)
        }
    }

    /// Total months of Operation across the authored segments (the window in
    /// which the objective can be worked).
    pub fn operation_months(&self) -> u32 {
        self.phases
            .iter()
            .filter(|p| p.kind == crate::data::contracts::ContractPhase::Operation)
            .map(|p| p.years * 12)
            .sum()
    }

    /// The segment index and phase kind for a given month of contract time.
    /// Month 0 is pre-launch Preparation; past the last segment is Completion.
    pub fn phase_at(&self, months: u32) -> (usize, crate::data::contracts::ContractPhase) {
        use crate::data::contracts::ContractPhase;
        if months == 0 {
            return (0, ContractPhase::Preparation);
        }
        let mut cumulative = 0;
        for (i, segment) in self.phases.iter().enumerate() {
            cumulative += segment.years * 12;
            if months <= cumulative {
                return (i, segment.kind);
            }
        }
        (
            self.phases.len().saturating_sub(1),
            ContractPhase::Completion,
        )
    }

    /// Index of the first Return segment, if the charter has one.
    pub fn first_return_index(&self) -> Option<usize> {
        self.phases
            .iter()
            .position(|p| p.kind == crate::data::contracts::ContractPhase::Return)
    }

    /// Cumulative month at which segment `i` begins.
    pub fn segment_start(&self, i: usize) -> u32 {
        self.phases[..i].iter().map(|s| s.years * 12).sum()
    }
}
