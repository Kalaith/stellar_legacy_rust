//! Ship subsystem catalog (W5): the six module families beyond hull/engine/
//! weapon, each buffering one event family through upgrade tiers. Identities and
//! balance live in `assets/subsystems.json`; no subsystem constants in Rust.

use crate::data::ResourceDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemTier {
    /// Drydock cost to reach this tier from the one below (authored positive;
    /// negated on spend).
    pub cost: ResourceDelta,
    /// 0-1: fraction of a matching event's negative deltas prevented at full
    /// condition and this tier.
    pub severity_reduction: f32,
    /// Multiplier on a matching event's roll weight (0.8 = 20% rarer).
    pub weight_multiplier: f32,
    /// In-universe log line when a drydock upgrade reaches this tier (content-
    /// depth subsystems round 5: tier-specific flavor, replacing one generic
    /// "rebuilt stronger" line shared by all 6 modules × 3 tiers). Empty falls
    /// back to the built-in line so the log is never blank.
    #[serde(default)]
    pub flavor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDef {
    pub id: String,
    pub name: String,
    /// Event family this subsystem buffers (matches `EventTemplate.family`, W6).
    /// Empty means it buffers no family — it acts only through its extra effect.
    pub buffers_family: String,
    pub decay_per_year: f32,
    /// Institutional knowledge (0-1) needed to repair it.
    pub repair_knowledge_required: f32,
    pub repair_parts_cost: i64,
    pub repair_minerals_cost: i64,
    /// Tier 0 is the ship's baseline (no entry here); `tiers[i]` is the upgrade
    /// to reach tier `i + 1`.
    pub tiers: Vec<SubsystemTier>,
    pub description: String,
}

impl SubsystemDef {
    /// The active tier's stats, or `None` at baseline tier 0 (no buffering).
    pub fn tier_stats(&self, tier: u32) -> Option<&SubsystemTier> {
        if tier == 0 {
            None
        } else {
            self.tiers.get(tier as usize - 1)
        }
    }
}

/// Subsystem tunables (W5). All balance lives here, never in Rust.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SubsystemsConfig {
    pub knowledge_start: f32,
    pub knowledge_decay_per_generation: f32,
    pub education_transmission_per_tier: f32,
    pub train_knowledge_gain: f32,
    pub train_cost_credits: i64,
    pub agriculture_food_bonus_per_tier: f32,
    /// Keystone coupling (content-depth subsystems round 7): the engineering bay
    /// is where the ship mends itself, so its condition modulates how fast every
    /// *other* module decays. The per-year decay multiplier is
    /// `1 + engineering_decay_swing * (0.5 - engineering_condition)` — a bay in
    /// top repair (cond 1.0) slows the rest of the ship's rot, a failing one
    /// (cond 0.0) speeds it, neutral at 0.5. 0 = no coupling. Engineering itself
    /// decays at its own rate.
    #[serde(default)]
    pub engineering_decay_swing: f32,
    /// Fraction of famine losses the medical bay itself prevents at full
    /// condition (content-depth subsystems round 9): the two modules that only
    /// ever *cost* the ship now earn their keep, and — unlike the tier-based
    /// bonuses — by how well they are *kept*. A bay in good repair saves more of
    /// the starving; it scales by condition and stacks with a serving medic
    /// (combined reduction capped). 0 = no bay-level relief.
    #[serde(default)]
    pub medical_famine_relief_per_condition: f32,
    /// Yearly unity recovery from a well-kept security/justice system at full
    /// condition (content-depth subsystems round 9), scaling by condition and
    /// stacking with a serving security chief. Only steadies a ship below the
    /// crew recovery ceiling. 0 = no bay-level recovery.
    #[serde(default)]
    pub security_unity_recovery_per_condition: f32,
    /// How much the habitat's state moves the ship's morale each year (content-depth
    /// subsystems round 11): the life-support/habitat is where the people *live*, so
    /// a home kept above the midpoint lifts spirits and one let to fail (cramped,
    /// cold, patched) depresses them. Applied as `swing * (condition - 0.5)`, the
    /// only maintenance-driven positive lever morale has against the voyage's strain.
    /// 0 = the habitat's state does not touch morale.
    #[serde(default)]
    pub habitat_morale_swing: f32,
    /// How much a module's tending-faction approval modulates its yearly decay
    /// (content-depth factions round 12): the reverse of the neglect-to-sentiment
    /// loop. A devoted people keeps its own domain sharp while a resentful one lets
    /// it slide. The per-year decay multiplier is `1 + scale·(0.5 - approval)`, so a
    /// devoted tender (approval near 1.0) slows that module's rot, a resentful one
    /// (near 0.0) speeds it, and a neutral one leaves it be. This closes the spiral
    /// where neglecting a module sours its tenders, who then let it rot faster
    /// still. 0 = a faction's mood does not touch upkeep.
    #[serde(default)]
    pub tender_approval_decay_scale: f32,
    /// How much a *degraded* agriculture bay cuts food production (content-depth
    /// subsystems round 12): the food module's missing condition→output coupling,
    /// the parallel to the medical/security condition effects. The yield factor is
    /// `1 - agriculture_condition_food_penalty·(1 - condition)`, so a pristine farm
    /// (condition 1.0) yields exactly as before while a rotting one feeds fewer —
    /// upkeep on the hydroponics paying back continuously, not only at breakdown.
    /// Penalty-below-full (not swing-around-half) so the launch baseline is
    /// untouched. 0 = a farm's condition does not touch its yield.
    #[serde(default)]
    pub agriculture_condition_food_penalty: f32,
    /// How much a *degraded* education/culture archive weakens generational
    /// knowledge transmission (content-depth subsystems round 13): the last module's
    /// missing condition→output coupling, and education's counterpart to the
    /// engineering keystone — where engineering's condition scales every module's
    /// *decay*, education's condition scales every module's *knowledge transfer*
    /// forward. The transmission factor is `1 - education_transmission_condition_penalty
    /// ·(1 - condition)`, so a vivid archive (condition 1.0) transmits fully — the
    /// untouched baseline — while a crumbling one loses more of the founding craft
    /// each generation. Penalty-below-full so the launch baseline is untouched.
    /// 0 = the archive's physical state does not touch what the next generation keeps.
    #[serde(default)]
    pub education_transmission_condition_penalty: f32,
    /// How much a *degraded* mission-key subsystem slows the objective's accrual
    /// (content-depth subsystems round 14): the subsystem axis's first coupling to
    /// the mission itself. A charter names the module its work leans on, and the
    /// on-station accrual is scaled by `1 - objective_condition_penalty·(1 - condition)`
    /// — a pristine bay works at the base rate, a rotting one slower. Penalty-below-
    /// full so a well-kept ship's objective is untouched. 0 = the module's state does
    /// not touch the work.
    #[serde(default)]
    pub objective_condition_penalty: f32,
    /// Condition below which a failing life-support/habitat plant begins to cost
    /// lives (content-depth subsystems round 15): the module's most fundamental
    /// effect, long missing — a plant that literally sustains the crew, when it
    /// fails badly, cannot sustain everyone. Above this the plant holds; below it,
    /// a yearly attrition scaled by how far it has failed. 0 = no mortality effect.
    #[serde(default)]
    pub life_support_failure_threshold: f32,
    /// Peak yearly fraction of the crew lost to a *fully collapsed* (condition 0)
    /// life-support plant (content-depth subsystems round 15), scaled linearly from
    /// 0 at the failure threshold to this at zero condition. Gentle — a slow
    /// thinning, the pressure to keep the plant alive, not a massacre.
    #[serde(default)]
    pub life_support_failure_mortality: f32,
}
