//! Founding faction definitions (W7): authored population groups a campaign
//! can carry aboard. Identities live in `assets/factions.json`; no faction
//! names or balance numbers appear in Rust source.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::data::events::SubsystemDelta;
use crate::data::PopulationDelta;

/// What a people brings aboard when recruited in drydock (content-depth factions
/// round 7: recruitable pool personalities). A one-time signature boon so *which*
/// faction you take on matters beyond a head count — the makers bring their
/// craft, the gardeners their green thumb. All effects are data; empty = a bare
/// recruit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecruitBoon {
    /// Signature subsystem lift (condition/knowledge) their expertise grants.
    pub subsystem_deltas: Vec<SubsystemDelta>,
    /// Signature population lift (morale/unity/etc.) their arrival brings.
    pub population_delta: PopulationDelta,
    /// The line narrating what they bring; empty falls back to the generic one.
    pub flavor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    /// -1.0 (tech-averse) .. +1.0 (tech-embracing). Unused mechanically in v1;
    /// reserved for the modifiers a later pass will layer on.
    pub ideology: f32,
    pub description: String,
    /// Short phrase used in logs, e.g. "the Verdant Kin".
    pub log_name: String,
    /// The dowry this people brings when recruited (content-depth round 7).
    #[serde(default)]
    pub recruit_boon: RecruitBoon,
    /// The subsystem this people's craft and identity are bound to (content-depth
    /// subsystems round 8): the makers to the engineering bay, the gardeners to
    /// agriculture, the Keepers to the culture archive. When their module is left
    /// to rot, they take it personally — its condition erodes their approval
    /// yearly (`SimState::apply_subsystem_neglect_sentiment`). Empty = no module
    /// this people answers for.
    #[serde(default)]
    pub tended_subsystem: String,
    /// Per-generation demographic drift (content-depth factions round 11): this
    /// people's share of the ship waxes or wanes each generation by this fraction,
    /// so the balance of power is not fixed at launch — a fecund people (the
    /// Hearth) grows toward the majority over centuries while a people that does
    /// not reproduce naturally (the augmented Ascension) dwindles, and *who runs
    /// the ship* — the lever behind drift, dilemmas, and event gates — can change
    /// mid-voyage. 0 = a stable people.
    #[serde(default)]
    pub growth_bias: f32,
    /// Peoples this one is at odds with (content-depth factions round 14): the
    /// friction pairs made a *persistent relationship*. When an event favors this
    /// people — lifting its approval — each aboard rival resents the favoritism and
    /// loses a fraction of it (`FactionConfig::rival_approval_spillover`), so the
    /// approval meter is a balancing act: you cannot please a people without a cost
    /// to those it quarrels with. Authored symmetric (if A names B, B names A).
    /// Empty = a people with no standing rivals.
    #[serde(default)]
    pub rivals: Vec<String>,
    /// Peoples this one is bound to in kinship (content-depth factions round 17):
    /// the positive twin of `rivals`. Where favoring a people *sours* its rivals,
    /// it *warms* its allies — when an event lifts this people's approval, each
    /// aboard ally shares a fraction of the goodwill
    /// (`FactionConfig::ally_approval_spillover`), so the approval meter is not only
    /// a set of fault lines but a landscape of coalitions: courting one people is a
    /// gift to its kin as much as a slight to its rivals. Authored symmetric (if A
    /// names B, B names A) and never overlapping a rivalry. Empty = a people that
    /// stands alone. (The natural kinships are the r5 merger pairs — the communal
    /// Hearth and the garden-tending Kin, the Accord's courts and the Covenant's
    /// forges — peoples the ship has folded into one before.)
    #[serde(default)]
    pub allies: Vec<String>,
    /// How this people, while it runs the ship, bends the ship's *reputation* traits
    /// (content-depth factions round 16): a per-trait lean strength (signed), so a
    /// communal, kind majority drifts the ship's `mercy` up over the generations and
    /// a coldly pragmatic one drifts it down — reputation built not only from event
    /// choices (it105) but from the standing character of who is in charge. Scaled by
    /// `dominant_reputation_lean_per_year`. Empty = a people whose rule leaves the
    /// ship's name to its choices alone.
    #[serde(default)]
    pub reputation_leanings: HashMap<String, f32>,
}

/// How a faction left the ship when an event drives it off (W7). WipedOut and
/// Assimilated arise from the simulation itself, not from an outcome, so they
/// are not represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionLossKind {
    Settled,
    Departed,
}

/// Faction tunables (W7). All balance lives here, never in Rust.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FactionConfig {
    /// How many factions a campaign founds with (the picker enforces it).
    pub starting_count: u32,
    /// Below this share of the people aboard, a faction may be assimilated…
    pub assimilation_share_threshold: f32,
    /// …but only once cultural drift has passed this.
    pub assimilation_drift_threshold: f32,
    pub recruit_group_cost_credits: i64,
    pub recruit_group_size: u32,
    /// Below this condition, a people watching its tended subsystem rot loses
    /// approval each year (content-depth subsystems round 8). 0 disables it.
    #[serde(default)]
    pub neglect_condition_threshold: f32,
    /// Approval a tending people sheds per year while its module sits below the
    /// neglect threshold. Gentle by design — sustained neglect, not one bad year,
    /// is what drives a people toward the door.
    #[serde(default)]
    pub neglect_approval_penalty: f32,
    /// How much a people's *approval* bends its per-generation demographic growth
    /// (content-depth factions round 13): the missing link between the approval
    /// meter (r8) and demographic drift (r11). Each generation a people's growth
    /// bias gains `approval_growth_factor · (approval − 0.5)`, so a beloved people
    /// (approval → 1) waxes toward the majority and a resented one (→ 0) wanes even
    /// beyond its base bias — how you treat a people decides not just whether it
    /// leaves but whether it grows or fades. 0 = approval does not touch growth.
    #[serde(default)]
    pub approval_growth_factor: f32,
    /// Fraction of a positive approval gain that a favored people's aboard *rivals*
    /// lose to resentment (content-depth factions round 14). Favoring one people
    /// sours those it quarrels with, so the approval meter cannot be maximized for
    /// everyone at once. 0 = rivalries do not spill over.
    #[serde(default)]
    pub rival_approval_spillover: f32,
    /// Fraction of a positive approval gain that a favored people's aboard *allies*
    /// also receive (content-depth factions round 17): the positive mirror of
    /// `rival_approval_spillover`. Favoring one people warms its kin, so the meter
    /// rewards building a coalition, not only pleasing an isolated people. 0 =
    /// kinships do not spill over.
    #[serde(default)]
    pub ally_approval_spillover: f32,
    /// How much the aboard peoples' mood moves the ship's `unity` each year
    /// (content-depth factions round 15): the faction system's first coupling to the
    /// ship's own cohesion. Unity drifts by `approval_unity_coupling · (mean_approval
    /// − 0.5)` — a content polity steadies the ship, a resentful one erodes it toward
    /// the crisis the beats watch for. 0 = faction mood does not touch cohesion.
    #[serde(default)]
    pub approval_unity_coupling: f32,
    /// Per-year drift the *dominant* people's `reputation_leanings` impart to the
    /// ship's reputation traits (content-depth factions round 16): the standing
    /// character of who runs the ship bleeds slowly into the ship's name. A trait
    /// drifts by `leaning · this` each year the leaning-holder is dominant. 0 = who
    /// rules does not touch reputation.
    #[serde(default)]
    pub dominant_reputation_lean_per_year: f32,
    /// Ideological spread (member-weighted MAD of aboard `ideology`) past which a
    /// divided polity begins to erode the ship's `stability` (content-depth factions
    /// round 18): the faction system's coupling to *governance*, distinct from the
    /// approval→unity one — a ship can be content yet fractious. A polity more unified
    /// than this is easy to govern (no drift); a coalition spanning the tech↔tradition
    /// spectrum past it strains the institutions. 0 threshold = the coupling is off.
    #[serde(default)]
    pub ideology_spread_threshold: f32,
    /// Stability drained per year per unit of ideological spread *above* the threshold
    /// (content-depth factions round 18). Gentle by design — the standing cost of
    /// holding a divided coalition together, which a well-kept security corps (it108)
    /// can counterbalance.
    #[serde(default)]
    pub ideology_spread_stability_penalty: f32,
    /// Subsystem *knowledge* a departing people takes with them (content-depth
    /// factions round 20): when a schism or a withdrawal loses a whole people, the
    /// module they tended (`FactionDef.tended_subsystem`) loses this much of its
    /// living expertise — the ones who truly understood it are gone. So shedding a
    /// people costs more than its headcount; it costs the craft it carried, feeding
    /// the knowledge-crisis events and the education keystone's slow re-teaching.
    /// Clamped at 0 knowledge. 0 = a departure takes no craft.
    #[serde(default)]
    pub departed_faction_knowledge_loss: f32,
}
