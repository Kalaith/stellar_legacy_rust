//! Session-scoped state: what is blocking the clock, how fast it runs, and
//! what the autoplayer is allowed to decide unattended.

use serde::{Deserialize, Serialize};

/// Per-category advisor delegation (GDD §5.4): a delegated category's events
/// auto-resolve via outcome scoring instead of blocking on the player.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DelegationSettings {
    pub immediate_crisis: bool,
    pub generational_challenge: bool,
    pub mission_milestone: bool,
    pub legacy_moment: bool,
}

impl DelegationSettings {
    pub fn is_delegated(&self, category: crate::data::events::EventCategory) -> bool {
        use crate::data::events::EventCategory::*;
        match category {
            ImmediateCrisis => self.immediate_crisis,
            GenerationalChallenge => self.generational_challenge,
            MissionMilestone => self.mission_milestone,
            LegacyMoment => self.legacy_moment,
        }
    }

    pub fn toggle(&mut self, category: crate::data::events::EventCategory) {
        use crate::data::events::EventCategory::*;
        match category {
            ImmediateCrisis => self.immediate_crisis = !self.immediate_crisis,
            GenerationalChallenge => self.generational_challenge = !self.generational_challenge,
            MissionMilestone => self.mission_milestone = !self.mission_milestone,
            LegacyMoment => self.legacy_moment = !self.legacy_moment,
        }
    }
}

/// An event waiting for a council decision. Stores the template id, not a
/// copy — the UI and resolver look the template up in `GameData`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEvent {
    pub template_id: String,
    /// Months-since-founding when the event fired (W3 month clock).
    pub rolled_month_clock: u32,
}

/// A follow-up event promised to fire at a determined voyage year (content-depth
/// event families round 9): the deterministic-timing counterpart to the
/// opportunistic `requires_consequence` chains. Queued by an outcome's
/// `schedule_followup`, fired once the voyage reaches `fire_year`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub template_id: String,
    /// Voyage year (years since founding) at or after which the follow-up fires.
    pub fire_year: u32,
}

/// A legacy dilemma waiting for a council decision (GDD §5.5). Stores the
/// dilemma id; the definition lives on the sim's legacy in `GameData`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDilemma {
    pub dilemma_id: String,
    /// Months-since-founding when the dilemma fired (W3 month clock).
    pub rolled_month_clock: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub year: u32,
    /// Calendar month 1-12 the line was stamped in (W3 month clock).
    pub month: u32,
    pub text: String,
}

/// Real-time voyage speed (real-time loop): while a mission is under way the
/// month clock auto-advances one month every `seconds_per_month / multiplier`
/// real seconds. `Paused` freezes time even mid-voyage; docked, time is frozen
/// regardless of this setting. Replaces the old manual per-press `SpeedStep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSpeed {
    Paused,
    #[default]
    X1,
    X2,
    X3,
}

impl GameSpeed {
    /// The selector row order: pause first, then the ascending rates.
    pub const ALL: [GameSpeed; 4] = [
        GameSpeed::Paused,
        GameSpeed::X1,
        GameSpeed::X2,
        GameSpeed::X3,
    ];

    /// Real-time multiplier on the auto-advance cadence (0 while paused). The
    /// baseline was rescaled so the slowest running speed matches the old top
    /// rate (1x ran too slow); the labels stay honest — 2x is twice 1x, 3x thrice.
    pub fn multiplier(self) -> f32 {
        match self {
            GameSpeed::Paused => 0.0,
            GameSpeed::X1 => 3.0,
            GameSpeed::X2 => 6.0,
            GameSpeed::X3 => 9.0,
        }
    }

    /// Short label for the speed-selector row.
    pub fn label(self) -> &'static str {
        match self {
            GameSpeed::Paused => "II",
            GameSpeed::X1 => "1x",
            GameSpeed::X2 => "2x",
            GameSpeed::X3 => "3x",
        }
    }
}
