//! The people aboard: how many, how long they live, and how badly a
//! failing system treats them.

use serde::{Deserialize, Serialize};

/// archetype; recruiting fills a vacancy, training raises the holder's
/// skill toward the archetype cap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewConfig {
    pub starting_posts: Vec<String>,
    pub recruit_cost_credits: i64,
    pub train_cost_credits: i64,
    pub train_skill_gain: u32,
    pub recruit_age_min: u32,
    pub recruit_age_max: u32,
    pub retirement_age: u32,
    /// Security-chief unity recovery only applies below this ceiling.
    pub unity_recovery_ceiling: f32,
}
/// Per-character mortality (real-time loop follow-up: characters age and die).
/// Aging is a shared "Founding Day" event — everyone gains a year on the last
/// day of the year, whatever their true birthdate — but *death* is a monthly
/// roll whose odds climb with age: a flat accident chance at any age, plus an
/// age-scaled term that switches on past `onset_age` and doubles every
/// `doubling_years`. Certain at `member_max_age`. A heavy population-loss event
/// can also claim a named crew officer or relative.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MortalityConfig {
    /// Age past which the age-scaled monthly death term switches on.
    pub onset_age: u32,
    /// Monthly death chance at `onset_age` (before the accident floor).
    pub monthly_base_chance: f32,
    /// Years over which the age-scaled term doubles.
    pub doubling_years: f32,
    /// Flat monthly death chance at any age (accidents, mishaps).
    pub monthly_accident_chance: f32,
    /// A population loss of at least this many souls in one outcome may also
    /// take a named character.
    pub event_death_loss_threshold: u32,
    /// Chance a qualifying population-loss event claims a named character.
    pub event_death_chance: f32,
    /// The dynasty size the line renews toward. Each Founding Day, while the
    /// dynasty sits below this and has at least two members to carry it on, new
    /// young adults come of age (see `annual_birth_chance`) — the counterweight to
    /// the death roll, so a healthy line churns individuals without dying out.
    pub dynasty_target_size: u32,
    /// Per open slot below `dynasty_target_size`, the yearly chance a new young
    /// adult comes of age. Higher fills a depleted line back up faster.
    pub annual_birth_chance: f32,
}
/// Thresholds and point values for the §5.5 failure-risk formula. Drift and
/// unity apply to every legacy; the rest gate on the matching legacy's
/// tracked counters (see `simulation::legacy::failure_risk`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FailureRiskConfig {
    pub drift_threshold: f32,
    pub drift_points: i32,
    pub unity_threshold: f32,
    pub unity_points: i32,
    pub tradition_threshold: i32,
    pub tradition_points: i32,
    pub body_horror_threshold: u32,
    pub body_horror_points: i32,
    pub dread_threshold: f32,
    pub dread_points: i32,
    pub piracy_threshold: f32,
    pub piracy_points: i32,
    pub at_risk_threshold: i32,
}
