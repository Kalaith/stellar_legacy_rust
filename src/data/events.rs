//! Event template definitions (GDD §5.4, §6).

use crate::data::factions::FactionLossKind;
use crate::data::{PopulationDelta, ResourceDelta, ShipDelta};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    ImmediateCrisis,
    GenerationalChallenge,
    MissionMilestone,
    LegacyMoment,
}

impl EventCategory {
    pub const ALL: [EventCategory; 4] = [
        EventCategory::ImmediateCrisis,
        EventCategory::GenerationalChallenge,
        EventCategory::MissionMilestone,
        EventCategory::LegacyMoment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EventCategory::ImmediateCrisis => "Immediate Crisis",
            EventCategory::GenerationalChallenge => "Generational Challenge",
            EventCategory::MissionMilestone => "Mission Milestone",
            EventCategory::LegacyMoment => "Legacy Moment",
        }
    }
}

/// A signed change to one named subsystem's condition and/or institutional
/// knowledge (content-depth iteration). Lets an event outcome wound a module,
/// patch it, teach a skill forward, or bury it with the experts who die.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDelta {
    pub id: String,
    #[serde(default)]
    pub condition: f32,
    #[serde(default)]
    pub knowledge: f32,
}

/// A crisis gate keyed to a subsystem stat falling to or below `below`
/// (content-depth): used for knowledge decay ("the last person who understood
/// the reactor is dying") and, since round 3, physical condition failure ("the
/// module is falling apart").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemGate {
    pub id: String,
    pub below: f32,
}

/// A subsystem-knowledge *floor* (content-depth event families round 12): the
/// mirror of a `SubsystemGate` — a threshold a module's living expertise must be
/// at or above, used to unlock an outcome only a ship that kept its experts sharp
/// can attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFloor {
    pub id: String,
    pub at_least: f32,
}

/// Availability gate on a single outcome (content-depth event families round 12):
/// the outcome-level parallel to the whole-event gates. An outcome carrying one is
/// offered only when the ship has *earned* it — a past choice on record, or a
/// subsystem's expertise kept high enough — so a crisis can present a better exit
/// to a prepared ship than to an unprepared one. Empty (the default) = always
/// offered. Gated outcomes are authored *after* the unconditional ones, so the
/// first outcome always shows and the auto-resolve/index contract is untouched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutcomeRequirement {
    /// Past choices that must be on record for this outcome to appear.
    #[serde(default)]
    pub requires_consequence: Vec<String>,
    /// Subsystem-knowledge floors: the outcome appears only while each named
    /// module's knowledge is at or above its threshold.
    #[serde(default)]
    pub min_knowledge: Vec<KnowledgeFloor>,
}

impl OutcomeRequirement {
    /// True when this gate names no requirement (the outcome always shows).
    pub fn is_unconditional(&self) -> bool {
        self.requires_consequence.is_empty() && self.min_knowledge.is_empty()
    }
}

/// A signed shift to a named faction's approval (content-depth factions round 8):
/// the coupling that lets an event choice earn or spend a people's goodwill.
/// Clamped to [0, 1] on apply; a no-op if that faction is not aboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionApprovalDelta {
    pub id: String,
    pub delta: f32,
}

/// A gate keyed to a faction's approval falling to or below `below` (content-depth
/// factions round 8): the event fires only while the named faction is aboard *and*
/// has soured to at least this degree — a grievance beat, or the withdrawal of a
/// people pushed too far.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionApprovalGate {
    pub id: String,
    pub below: f32,
}

/// A follow-up an outcome promises to fire at a determined future year
/// (content-depth event families round 9): the deterministic-timing counterpart
/// to the opportunistic `long_term_consequences`/`requires_consequence` chain.
/// The named event is forced (bypassing its gates) once the voyage has advanced
/// `delay_years` from the choice — a reckoning kept on a clock, not left to the RNG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledFollowup {
    pub template_id: String,
    pub delay_years: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventOutcome {
    pub id: String,
    pub label: String,
    pub description: String,
    /// Availability gate (content-depth event families round 12): when set, this
    /// outcome is offered only to a ship that has earned it (a past consequence, a
    /// subsystem kept expert). Empty = always offered. See `OutcomeRequirement`.
    #[serde(default)]
    pub requires: OutcomeRequirement,
    #[serde(default)]
    pub resource_delta: ResourceDelta,
    #[serde(default)]
    pub ship_delta: ShipDelta,
    #[serde(default)]
    pub population_delta: PopulationDelta,
    /// Named consequences recorded on the sim for later event weighting
    /// (Pillar 2: debts someone else pays). Each entry also costs 100 points
    /// in auto-resolve outcome scoring (GDD §5.4).
    #[serde(default)]
    pub long_term_consequences: Vec<String>,
    /// A ship component id this outcome drops into the salvage hold (PLAN M4.4).
    #[serde(default)]
    pub grant_component: Option<String>,
    /// When set, an applied outcome turns an active mission for home early (W2):
    /// the contract jumps to its Return segment. Fits both catastrophe (a crisis
    /// forcing withdrawal) and fortune (a find rich enough to sail back on).
    #[serde(default)]
    pub force_return: bool,
    /// When set, an applied outcome loses the ship's smallest faction this way
    /// (W7) — they settled off-ship or departed. Skipped if only one remains.
    #[serde(default)]
    pub faction_loss: Option<FactionLossKind>,
    /// With `faction_loss` set, loses this *specific* faction instead of the
    /// smallest (content-depth round 3: faction-specific schism beats). Ignored
    /// when `faction_loss` is `None`; a no-op if that faction is already gone.
    #[serde(default)]
    pub faction_loss_id: Option<String>,
    /// Merge this named faction into the largest other aboard (content-depth
    /// round 5: event-driven assimilation beats — the *union* counterpart to
    /// `faction_loss_id`'s schism). Its people stay aboard and keep the head
    /// count; only the separate identity dissolves. No-op if it is not aboard or
    /// is the ship's last people. Independent of `faction_loss`.
    #[serde(default)]
    pub faction_merge_id: Option<String>,
    /// Signed changes to named subsystems (content-depth iteration): condition
    /// and/or knowledge, clamped to [0, 1]. This is the coupling that lets an
    /// engineering crisis actually damage the engineering bay, or a teaching
    /// succession restore its lost know-how.
    #[serde(default)]
    pub subsystem_deltas: Vec<SubsystemDelta>,
    /// Signed shifts to named factions' approval (content-depth factions round 8):
    /// how an outcome earns or spends a people's goodwill. Aboard factions only.
    #[serde(default)]
    pub faction_approval_deltas: Vec<FactionApprovalDelta>,
    /// Signed shift to the *smallest* aboard faction's approval (content-depth
    /// provisioning round 8): the dynamic "who bears the cut" of a shortage triage,
    /// resolved at apply-time so a general rationing beat need not name a people.
    /// 0.0 = no shift.
    #[serde(default)]
    pub faction_approval_smallest: f32,
    /// Signed change to the active charter's objective progress, as a *fraction of
    /// its target* (content-depth provisioning round 9): the coupling that lets
    /// the founders' mission and the living's survival compete — diverting the
    /// work crews to forage in a famine feeds the ship but slips the tally.
    /// Applied (clamped ≥ 0) only while a contract is under way. 0.0 = no change.
    #[serde(default)]
    pub objective_progress_delta: f32,
    /// A follow-up promised to fire at a determined future year (content-depth
    /// event families round 9). `None` = no scheduled payoff.
    #[serde(default)]
    pub schedule_followup: Option<ScheduledFollowup>,
    #[serde(default)]
    pub log: String,
}

/// A state-gated twist that can ride along on an event (content-depth event
/// families round 6): "an event that can arrive with 2–3 complications is worth
/// three flat events." When its gates pass, the complication's `description_add`
/// is appended to the event as shown, and — whichever outcome the player takes —
/// its deltas and `log` land on top. The sim is paused while an event blocks, so
/// the same complication resolves at present-time and apply-time from identical
/// state; no stored field, and the outcome list is untouched. At most one
/// complication (the first whose gates pass, in authored order) rides at a time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Complication {
    pub id: String,
    /// Gates (all must hold): the drift the people have reached, subsystems
    /// physically failing, prior consequences recorded, a food shortage. Empty /
    /// 0.0 / None = that gate ignored.
    #[serde(default)]
    pub min_cultural_drift: f32,
    #[serde(default)]
    pub condition_below: Vec<SubsystemGate>,
    #[serde(default)]
    pub requires_consequence: Vec<String>,
    #[serde(default)]
    pub food_below: Option<i64>,
    /// Faction gates (content-depth factions round 6: faction-colored event
    /// reactions). The complication rides only while this faction runs the ship
    /// (largest aboard) and/or every listed faction is still aboard — so the same
    /// crisis reads and plays differently depending on who is in charge. Empty =
    /// that gate ignored.
    #[serde(default)]
    pub requires_dominant_faction: String,
    #[serde(default)]
    pub requires_factions_aboard: Vec<String>,
    /// Recurrence gate (content-depth event families round 11): the complication
    /// rides only once this same event has already fired at least this many times
    /// this campaign — a recurring crisis that escalates instead of repeating.
    /// 0 = no recurrence requirement.
    #[serde(default)]
    pub min_prior_occurrences: u32,
    /// Lived-state gates (content-depth event families round 15): the complication
    /// rides only while the crew has thinned to or below `max_population` and/or the
    /// shortage has ground on for at least `min_lean_food_years` years — so a crisis
    /// reads and bites differently on a skeleton crew or a ship worn thin by decades
    /// of want, not just by who runs it or how far it has drifted. 0 = that gate
    /// ignored.
    #[serde(default)]
    pub max_population: u32,
    #[serde(default)]
    pub min_lean_food_years: u32,
    /// Choice targeting (content-depth event families round 14): when non-empty,
    /// the complication's extra toll and log land *only* if the chosen outcome's id
    /// is listed here — so a twist can punish a *specific* decision (an unstable
    /// reactor makes *pushing through* worse but leaves *scramming* alone) rather
    /// than every choice alike. The `description_add` still always shows (the twist
    /// is visible before the choice). Empty = the toll lands on whichever outcome is
    /// taken (the round-6 behavior).
    #[serde(default)]
    pub applies_to_outcomes: Vec<String>,
    /// Sentence appended to the event's description when the complication rides.
    pub description_add: String,
    /// Extra consequences applied on top of the chosen outcome, and the line that
    /// narrates them.
    #[serde(default)]
    pub resource_delta: ResourceDelta,
    #[serde(default)]
    pub ship_delta: ShipDelta,
    #[serde(default)]
    pub population_delta: PopulationDelta,
    #[serde(default)]
    pub subsystem_deltas: Vec<SubsystemDelta>,
    #[serde(default)]
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTemplate {
    pub id: String,
    pub category: EventCategory,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub requires_decision: bool,
    /// Event family (W5, filled by W6). Matches a subsystem's `buffers_family`
    /// so the right module can soften or rarefy it. Empty = untagged.
    #[serde(default)]
    pub family: String,
    /// Contract phases this event may fire in (W6). Empty = any phase.
    #[serde(default)]
    pub phases: Vec<crate::data::contracts::ContractPhase>,
    /// Voyage gates (W6): the event stays out of the pool until the campaign has
    /// reached these. 0 / 0.0 = ungated.
    #[serde(default)]
    pub min_year: u32,
    #[serde(default)]
    pub min_generation: u32,
    #[serde(default)]
    pub min_cultural_drift: f32,
    /// Well-being gate (content-depth campaign-skeleton round 8): the event only
    /// enters the pool while the people's `morale` is at or above this — the
    /// honest gate for golden-age/flourishing content, so a celebration never
    /// surfaces on a miserable ship (whether forced by a flourish beat or rolled).
    /// 0.0 = ungated.
    #[serde(default)]
    pub min_morale: f32,
    /// Cohesion gate (content-depth campaign-skeleton round 13): the event only
    /// enters the pool while the people's `unity` is at or above this — the honest
    /// gate for recovery/reconciliation content (the cohesion twin of `min_morale`),
    /// so "the ship pulled back together" cannot surface on a fracturing one,
    /// whether forced by a recovery beat or rolled. 0.0 = ungated.
    #[serde(default)]
    pub min_unity: f32,
    /// Founder-authority gate (content-depth campaign-skeleton round 14): the event
    /// only enters the pool while the people's `legacy_loyalty` has fallen to or
    /// below this — the honest gate for covenant-lapse content, so "the founders no
    /// longer bind us" cannot surface on a still-devoted ship, whether forced by a
    /// loyalty-collapse beat or rolled. 0.0 = ungated.
    #[serde(default)]
    pub max_legacy_loyalty: f32,
    /// Mission-progress gate (content-depth campaign-skeleton round 9): the event
    /// only enters the pool while an active charter's objective is at or past this
    /// fraction — the honest gate for milestone content, so "the work is half
    /// done" cannot surface before it is. 0.0 = ungated (requires a contract when
    /// > 0).
    #[serde(default)]
    pub min_objective_fraction: f32,
    /// Depopulation gate (content-depth campaign-skeleton round 12): the event only
    /// enters the pool while the crew has fallen to or below this *headcount* — the
    /// honest gate for crew-thinning content (the descending mirror of `min_morale`),
    /// so "the decks stand half empty" cannot surface on a full ship, whether forced
    /// by a depopulation beat or rolled. An absolute count (founding is ~1000).
    /// 0 = ungated.
    #[serde(default)]
    pub max_population: u32,
    /// Chronic-scarcity gate (content-depth provisioning round 13): the event only
    /// enters the pool once the food store has sat below the lean line for at least
    /// this many consecutive years (`sim.lean_food_years`) — the honest gate for
    /// *long-hunger* content, so a beat about a lean generation cannot surface on a
    /// ship one bad winter into a shortage. Pair with `food_below` for a currently
    /// lean ship. 0 = ungated.
    #[serde(default)]
    pub min_lean_food_years: u32,
    /// Sustained-plenty gate (content-depth provisioning round 14): the mirror of
    /// `min_lean_food_years` — the event only enters the pool once the food store has
    /// sat at or above the fat line for at least this many consecutive years
    /// (`sim.fat_food_years`), the honest gate for *soft-generation* content, so a
    /// beat about a people raised never knowing want cannot surface on a ship one
    /// good harvest into plenty. 0 = ungated.
    #[serde(default)]
    pub min_fat_food_years: u32,
    /// Scheduled-only (content-depth event families round 9): the event never
    /// enters a random or beat roll — it fires solely as the timed payoff of a
    /// `schedule_followup`. Keeps a determined-clock reckoning out of the reactive
    /// pool so it lands exactly when promised, and only then.
    #[serde(default)]
    pub scheduled_only: bool,
    /// Era ceilings (content-depth campaign-skeleton round 4): the mirror of the
    /// `min_*` gates — the event leaves the pool once the voyage passes these, so
    /// content that belongs to a voyage era (e.g. the deep-middle "the ship is
    /// the only world" beats) can bow out before homecoming rather than leaking
    /// into it. 0 = no ceiling.
    #[serde(default)]
    pub max_year: u32,
    #[serde(default)]
    pub max_generation: u32,
    /// Consequence chain gate (content-depth iteration): the event stays out of
    /// the pool until a prior outcome has recorded every tag listed here in
    /// `sim.consequences`. This is how an early choice re-fires a consequence
    /// decades later — the payoff half of `EventOutcome::long_term_consequences`.
    /// Empty = ungated.
    #[serde(default)]
    pub requires_consequence: Vec<String>,
    /// Consequence *bar* (content-depth event families round 13): the negative twin
    /// of `requires_consequence` — the event stays out of the pool if *any* tag
    /// listed here is on record. So a choice can permanently *close a door*: a
    /// windfall of trust never offered to a ship known to have broken its word, a
    /// founding reverence impossible for a ship that buried its founding truth.
    /// Empty = nothing bars it.
    #[serde(default)]
    pub forbidden_consequence: Vec<String>,
    /// Charter-tag gate (content-depth iteration): the event only enters the
    /// pool while an active contract carries every tag listed here (see
    /// `ContractTemplate::tags`). This lets a destination color its own event
    /// pool — hostile space breeds boarding scares, garden runs breed settlers.
    /// Empty = any charter (or none).
    #[serde(default)]
    pub requires_charter_tag: Vec<String>,
    /// Faction-colored gate (content-depth iteration): the event only fires
    /// while this faction is the largest aboard — its signature situations
    /// surface when it runs the ship. Empty = any dominant faction.
    #[serde(default)]
    pub requires_dominant_faction: String,
    /// Inter-faction friction gate: every faction id listed must still be
    /// aboard for the event to fire (e.g. a quarrel between two rival groups).
    /// Empty = no faction-presence requirement.
    #[serde(default)]
    pub requires_factions_aboard: Vec<String>,
    /// Faction-approval gate (content-depth factions round 8): the event fires
    /// only while every named faction is aboard *and* soured to or below its
    /// threshold — the grievance/withdrawal beats a mistreated people generate.
    /// Empty = ungated.
    #[serde(default)]
    pub faction_approval_below: Vec<FactionApprovalGate>,
    /// Knowledge-crisis gates (content-depth iteration): the event only fires
    /// while every listed subsystem's knowledge has decayed to or below its
    /// threshold. Empty = ungated.
    #[serde(default)]
    pub knowledge_below: Vec<SubsystemGate>,
    /// Condition-breakdown gates (content-depth round 3): the event only fires
    /// while every listed subsystem's physical condition has fallen to or below
    /// its threshold — the module is breaking down, not just forgotten. Empty =
    /// ungated.
    #[serde(default)]
    pub condition_below: Vec<SubsystemGate>,
    /// Provisioning-shortage gates (content-depth iteration): the event only
    /// enters the pool while the ship is actually short — food store at or below
    /// `food_below`, fuel fraction at or below `fuel_below`, spare parts at or
    /// below `spare_parts_below`. `None` = that resource ungated. This is what
    /// makes a garden-world stop or a fuel-skim read as a *consequence* of
    /// running low rather than a random roll.
    #[serde(default)]
    pub food_below: Option<i64>,
    #[serde(default)]
    pub fuel_below: Option<f32>,
    #[serde(default)]
    pub spare_parts_below: Option<i64>,
    #[serde(default)]
    pub energy_below: Option<i64>,
    /// Provisioning-*abundance* gates (content-depth provisioning round 11): the
    /// inverse of the shortage set — the event only enters the pool while the ship
    /// is genuinely flush, food store at or above `food_above`, treasury at or
    /// above `credits_above`. `None` = that resource ungated. The first gates that
    /// key on *plenty* rather than want, so a fat-years choice reads as a
    /// consequence of prosperity rather than a random roll.
    #[serde(default)]
    pub food_above: Option<i64>,
    #[serde(default)]
    pub credits_above: Option<i64>,
    /// Multiplier on this template's roll weight per legacy id (GDD §6).
    #[serde(default)]
    pub legacy_weight_modifiers: HashMap<String, f32>,
    pub outcomes: Vec<EventOutcome>,
    /// State-gated twists this event can arrive with (content-depth round 6).
    /// Empty = the event always plays flat.
    #[serde(default)]
    pub complications: Vec<Complication>,
}
