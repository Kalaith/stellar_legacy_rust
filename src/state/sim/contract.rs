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
    /// Months in which the food store sat above its crisis threshold — one half
    /// of the ResourceEfficiency metric, accrued over the whole voyage.
    #[serde(default)]
    pub healthy_food_months: u32,
    /// Months in which the energy store sat above its crisis threshold — the
    /// other half of the ResourceEfficiency metric.
    #[serde(default)]
    pub healthy_energy_months: u32,
    /// Destination/mission tags copied from the charter at launch
    /// (content-depth iteration). Events gate on these via
    /// `EventTemplate::requires_charter_tag`.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Charter beat-pool override (content-depth charters round 7): extra event
    /// families layered into *every* seeded beat's draw for this voyage, so a
    /// charter biases the campaign it generates — an embassy leans diplomacy, a
    /// derelict recovery leans mystery. Copied from the charter at launch. Empty
    /// = no bias (the phase/era pools alone).
    #[serde(default)]
    pub beat_families: Vec<String>,
    /// How many cultural-drift threshold beats have fired so far (content-depth
    /// round 2). Thresholds are ascending, so this doubles as the index of the
    /// next threshold to watch — each drift beat fires exactly once.
    #[serde(default)]
    pub drift_beats_fired: u32,
    /// How many adaptation-threshold beats have fired (content-depth round 3),
    /// the physiological parallel to `drift_beats_fired`.
    #[serde(default)]
    pub adaptation_beats_fired: u32,
    /// How many cohesion-collapse crisis beats have fired (content-depth round 6):
    /// the *descending* mirror of the drift/adaptation beats. Thresholds descend,
    /// so this doubles as the index of the next (lower) unity level to watch —
    /// each crisis beat fires once as the ship comes apart.
    #[serde(default)]
    pub crisis_beats_fired: u32,
    /// How many anniversary beats have fired (content-depth round 7): the
    /// periodic commemoration cadence. Doubles as the count of anniversaries
    /// observed, so the next fires when the voyage passes the following multiple.
    #[serde(default)]
    pub anniversaries_fired: u32,
    /// How many golden-age flourish beats have fired (content-depth round 8): the
    /// *ascending* positive pole of the crisis beats. Thresholds ascend, so this
    /// doubles as the index of the next (higher) morale level to watch — each
    /// fires once as a thriving ship climbs into its golden years.
    #[serde(default)]
    pub flourish_beats_fired: u32,
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

    /// Fraction of voyage months the upkeep stores (food, energy) spent above
    /// their crisis thresholds — provisioning discipline measured across the
    /// whole contract, not an instant snapshot. 1.0 before any month elapses.
    pub fn upkeep_health(&self) -> f32 {
        if self.months_elapsed == 0 {
            1.0
        } else {
            (self.healthy_food_months + self.healthy_energy_months) as f32
                / (2 * self.months_elapsed) as f32
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

    /// How many times a phase of `kind` has been entered by the current segment
    /// (1-based), for occurrence-aware phase-transition flavor (voice round 3):
    /// the first Travel returns 1, a double-hop's second Travel returns 2.
    pub fn phase_occurrence(&self, kind: crate::data::contracts::ContractPhase) -> usize {
        let upto = self.phase_index.min(self.phases.len().saturating_sub(1));
        self.phases[..=upto]
            .iter()
            .filter(|p| p.kind == kind)
            .count()
            .max(1)
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
