//! Contract (mission) objective templates (GDD §5.2, §6).

use crate::data::{PopulationDelta, ResourceDelta, ShipDelta};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractObjective {
    Mining,
    Colonization,
    Exploration,
    Rescue,
    /// A residency among a living people, graded in standing/accords (content-depth
    /// charters round 8). An embassy is not a rescue — this gives the charter card
    /// the right word for a mission whose "yield" is a relationship.
    Diplomacy,
    /// Recovering mass from a wreck or derelict (content-depth charters round 8):
    /// a salvage haul is not mining — no seam is worked, a dead ship is stripped.
    Salvage,
}

impl ContractObjective {
    pub fn label(self) -> &'static str {
        match self {
            ContractObjective::Mining => "Mining",
            ContractObjective::Colonization => "Colonization",
            ContractObjective::Exploration => "Exploration",
            ContractObjective::Rescue => "Rescue",
            ContractObjective::Diplomacy => "Diplomacy",
            ContractObjective::Salvage => "Salvage",
        }
    }
}

/// preparation -> travel -> operation -> return -> completion (GDD §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractPhase {
    Preparation,
    Travel,
    Operation,
    Return,
    Completion,
}

impl ContractPhase {
    pub fn label(self) -> &'static str {
        match self {
            ContractPhase::Preparation => "Preparation",
            ContractPhase::Travel => "Travel",
            ContractPhase::Operation => "Operation",
            ContractPhase::Return => "Return",
            ContractPhase::Completion => "Completion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    PopulationSurvival,
    MissionCompletion,
    ResourceEfficiency,
    SocialCohesion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDef {
    pub id: String,
    pub kind: MetricKind,
    pub name: String,
    /// Weights across a template's metrics should sum to 1.0.
    pub weight: f32,
    pub target: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneDef {
    pub id: String,
    pub name: String,
    /// 0-1 progress fraction at which this milestone is reached.
    pub progress_threshold: f32,
    /// One-time resources granted when this milestone is first reached
    /// (PLAN item 3 — progress pays off along the way).
    #[serde(default)]
    pub reward: ResourceDelta,
}

/// One authored segment of a charter's fixed timeline (W2). The charter, never
/// the player, sets phase lengths; the years across a charter's segments sum
/// exactly to its `target_duration_years`. Only Travel / Operation / Return are
/// valid here — Preparation and Completion are the pre-launch and post-return
/// bookends, not authored segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseDef {
    pub kind: ContractPhase,
    pub years: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTemplate {
    pub id: String,
    pub name: String,
    pub objective: ContractObjective,
    pub description: String,
    pub target_duration_years: u32,
    /// Authored travel → operation → return timeline (W2). Sums to
    /// `target_duration_years`.
    pub phases: Vec<PhaseDef>,
    /// Quantified objective amount the mission must reach for full pay (W2):
    /// mine X, land X, chart X. Accrued only during Operation; pay is strictly
    /// proportional to the fraction of this reached. For a *preserve* charter (below)
    /// this is instead the amount the ship sets out *carrying*, and pay is the fraction
    /// that survives to arrival.
    pub objective_target: f32,
    /// Human unit for the objective counter ("proof-of-yield cores", "settlers
    /// landed", "systems charted", "souls recovered").
    pub objective_unit: String,
    /// A fundamentally different objective *shape* (content-depth charters round 23): an
    /// objective the ship does not *build* but *keeps*. Where an ordinary charter accrues
    /// its objective from zero during Operation, a preserve charter sets out *carrying*
    /// the full `objective_target` — frozen colonists, refugees, a fragile cargo — and
    /// the mission is to arrive with as much of it intact as possible. Its objective does
    /// not accrue; it only *erodes*, at `preserve_attrition_per_year` over the whole
    /// voyage (the cold banks fail, the sick do not all wake), plus whatever hazard events
    /// take. Pay is the surviving fraction. false = an ordinary accruing objective.
    #[serde(default)]
    pub preserve_objective: bool,
    /// Fraction of the carried objective lost per voyage-year on a preserve charter
    /// (round 23): the steady attrition of a fragile cargo across the long dark, applied
    /// every month of Travel/Operation/Return. Gentle — the mission is to minimise a loss
    /// that cannot be wholly stopped. Ignored unless `preserve_objective`.
    #[serde(default)]
    pub preserve_attrition_per_year: f32,
    pub milestones: Vec<MilestoneDef>,
    pub success_metrics: Vec<MetricDef>,
    #[serde(default)]
    pub failure_risks: Vec<String>,
    #[serde(default)]
    pub reward: ResourceDelta,
    /// Chronicle renown a dynasty must have accrued before this charter unlocks
    /// (PLAN M4.8). 0 = available from the founding; richer charters gate higher.
    #[serde(default)]
    pub min_renown: i64,
    /// Free-form destination/mission tags (content-depth iteration) copied onto
    /// the active contract at launch. Events may gate on them via
    /// `EventTemplate::requires_charter_tag`, so a charter colors which events
    /// its voyage can surface (e.g. `hostile_space`, `garden`, `long_haul`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Beat-pool bias (content-depth charters round 7): extra event families
    /// layered into every seeded beat's draw for this voyage, so the charter
    /// shapes the campaign it generates. Must be real families with events.
    /// Empty = the standard phase/era pools only.
    #[serde(default)]
    pub beat_families: Vec<String>,
    /// Scripted timed beats (content-depth charters round 9): specific events this
    /// charter forces at determined voyage years, so a mission can be built around
    /// a reckoning on a *known clock* — a predicted stellar event, a treaty
    /// deadline. Each names a `scheduled_only` event; `at_year` is years since
    /// launch and the list must ascend. Empty = no scripted beats.
    #[serde(default)]
    pub scheduled_beats: Vec<ScheduledBeat>,
    /// Route hazard (content-depth charters round 11): the charter's *risk
    /// profile*, added to the immediate-crisis category weight for the whole
    /// voyage — a lawless derelict field or a star's reach breeds more crises than
    /// a quiet survey, so a dangerous writ *feels* dangerous (and pays for it in
    /// its reward). 0 = an ordinary route.
    #[serde(default)]
    pub hazard: f32,
    /// In-world availability gate (content-depth charters round 12): a writ offered
    /// only while these founding peoples are aboard *right now* — the charter-level
    /// parallel to the outcome gates, and the in-world twin of `min_renown`'s
    /// cross-campaign fame. So a mission a people is uniquely trusted with or called
    /// to appears only on a ship that carries them, and vanishes if they leave.
    /// Empty = offered to any ship (subject to renown).
    #[serde(default)]
    pub requires_faction_aboard: Vec<String>,
    /// Per-year toll the route exacts for its whole duration (content-depth charters
    /// round 13): hazard's deterministic companion. Where `hazard` breeds more
    /// crises (a stochastic danger), this is a steady drain applied every year of
    /// the voyage, so a route whose *nature* wears at a ship — a star's radiation
    /// bath, the grim haunt of a ship of the dead — feels that way continuously
    /// rather than only in the crises it throws. Default (all-zero) = no toll.
    #[serde(default)]
    pub annual_toll: AnnualToll,
    /// In-world availability gate on the ship's *deeds* (content-depth charters
    /// round 14): the writ is offered only once every listed consequence is on
    /// record — how a **charter arc** is built, so completing one mission unlocks
    /// the follow-on. The consequence-twin of `requires_faction_aboard`. Empty =
    /// no deed required.
    #[serde(default)]
    pub requires_consequence: Vec<String>,
    /// The negative twin (content-depth charters round 14): the writ is barred if
    /// *any* listed consequence is on record — delicate work a ship's dark history
    /// disqualifies it from. Empty = nothing bars it.
    #[serde(default)]
    pub forbidden_consequence: Vec<String>,
    /// Consequence recorded when this charter is seen through to full term
    /// (content-depth charters round 14): the seed of a **charter arc** — a survey
    /// completed proves the ground a later colony writ needs. Empty = the charter
    /// leaves no such mark.
    #[serde(default)]
    pub completion_consequence: String,
    /// The subsystem this mission's work leans on (content-depth subsystems round
    /// 14): the module whose condition scales how fast the objective accrues
    /// on-station — a mining survey's engineering bay, a greening's agriculture, a
    /// science dive's education archive. A well-kept module works the mission faster,
    /// a rotting one slower, so the subsystem axis at last touches the objective the
    /// voyage exists for. Empty = the objective is indifferent to any module's state.
    #[serde(default)]
    pub objective_subsystem: String,
    /// How the ship's *combat rating* quickens this mission's objective (content-depth
    /// charters round 21 — the charter↔ship-loadout coupling). Until now the ship's
    /// aggregate combat did exactly one thing (back a Wanderer's defining gamble,
    /// `dilemma_odds`); the drydock's weapon-fitting decision never touched a mission.
    /// On a *contested* writ — a salvage on a wreck-line picked over by scavengers, a
    /// run through hostile space — an armed ship holds the field and works the objective
    /// openly while an unarmed one works furtive, in snatches, always ready to run: each
    /// point of combat adds this fraction to the accrual rate, exactly as `speed` does,
    /// but *only* where the mission's nature rewards firepower (a quiet survey sets 0, so
    /// guns are dead weight there). So the guns you fit in dock now decide which missions
    /// go *well*, not only which dilemmas you win. 0 = an objective indifferent to arms.
    #[serde(default)]
    pub objective_combat_scaling: f32,
    /// How the ship's *cargo capacity* quickens this mission's objective (content-depth
    /// charters round 24 — the third and last loadout↔charter accrual lever, after speed
    /// and combat r21). Until now the ship's aggregate `cargo` did one passive thing (a
    /// yearly minerals trickle from bigger holds); it never touched a *mission*. On a
    /// **haul** writ — a mining run measured in tonnes, a salvage stripping a wreck — the
    /// hold *is* the bottleneck: a big-bellied ship carries more out of every operation
    /// cycle while a small one fills and must stop. Each point of cargo adds this fraction
    /// to the accrual rate, exactly as combat does, but *only* where the objective is a
    /// quantity of material (a survey or a greening sets 0 — a hold hauls no readings and
    /// lands no settlers faster). So the drydock's hold-vs-guns-vs-engine trade now shapes
    /// which missions each fitting flies well. 0 = an objective indifferent to hold size.
    #[serde(default)]
    pub objective_cargo_scaling: f32,
    /// Reputation gates on the writ (content-depth charters round 16): the mission
    /// is offered only while the ship's cumulative character meets every named trait
    /// — at or above (`min_reputation`) or at or below (`max_reputation`) its
    /// threshold. So a relief writ opens only to a hull famous for mercy, and cold
    /// work only to one known not to flinch. Empty = ungated by reputation.
    #[serde(default)]
    pub min_reputation: Vec<crate::data::events::ReputationGate>,
    #[serde(default)]
    pub max_reputation: Vec<crate::data::events::ReputationGate>,
    /// A lasting boon granted when this charter is seen through to full term
    /// (content-depth charters round 15): the mission's *legacy*, distinct from its
    /// pro-rated pay. Building a great extraction works makes the ship's engineers
    /// permanently better; greening a world deepens its gardeners' craft. Applied in
    /// the conclude path once, alongside the completion mark. Default (empty) = the
    /// charter leaves nothing but its pay.
    #[serde(default)]
    pub completion_reward: CompletionReward,
    /// The mark a charter leaves when it is *not* seen through — concluded at Failure,
    /// defaulted or given up (content-depth charters round 18): the negative mirror of
    /// `completion_reward`, and the first charter effect keyed to *failure* rather than
    /// success. A relief run abandoned hardens the mercy the crew could not keep; any
    /// writ quit half-done earns the ship a name for folding (`resolve`) — the ultimate
    /// yielding. Applied once in the conclude path when the level is Failure (a
    /// non-failing conclusion instead earns the completion mark and reward). Default
    /// (empty) = a charter whose failure costs only its pay.
    #[serde(default)]
    pub abandonment: Abandonment,
}

/// The mark a defaulted or abandoned charter leaves on the ship's *name* (content-depth
/// charters round 18): the negative mirror of `CompletionReward`, applied when a charter
/// concludes at Failure. Chiefly reputation — a name for folding, a hardened mercy — with
/// the line that narrates it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Abandonment {
    #[serde(default)]
    pub reputation_deltas: Vec<crate::data::events::ReputationDelta>,
    /// Line narrating the default; empty = a generic line.
    #[serde(default)]
    pub log: String,
}

impl Abandonment {
    /// True when a failed charter costs the ship's name nothing (only its pay).
    pub fn is_none(&self) -> bool {
        self.reputation_deltas.is_empty()
    }
}

/// The lasting capability a charter grants on completion (content-depth charters
/// round 15): chiefly subsystem boons — a skill the ship keeps across voyages —
/// with optional resource/population lifts and the line that narrates it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionReward {
    #[serde(default)]
    pub subsystem_deltas: Vec<crate::data::events::SubsystemDelta>,
    #[serde(default)]
    pub resource: ResourceDelta,
    #[serde(default)]
    pub population: PopulationDelta,
    /// How seeing this charter through shapes the ship's *character* (content-depth
    /// charters round 17): a whole voyage of carrying refugees to safety deepens the
    /// ship's mercy; one spent on cold enforcement hardens it. So the missions a
    /// reputation *unlocks* (it106) then *build that reputation further* — a
    /// self-reinforcing spiral through the mission cycle. Applied on full completion.
    #[serde(default)]
    pub reputation_deltas: Vec<crate::data::events::ReputationDelta>,
    /// How seeing this charter through earns the goodwill of the peoples it served
    /// (content-depth charters round 19): a greening the Verdant Kin longed for, a
    /// homeworld run the traditionalists blessed — completing the mission lifts the
    /// named aboard factions' approval. So a mission the ship took *because* a people
    /// was aboard (it112 `requires_faction_aboard`) can leave that people delighted,
    /// which then unlocks the round-19 `faction_approval_above` gift beats: the
    /// charter cycle feeding the faction-goodwill well. Aboard factions only.
    #[serde(default)]
    pub faction_approval_deltas: Vec<crate::data::events::FactionApprovalDelta>,
    /// A ship component this charter recovers (content-depth charters round 20): a
    /// mission seen through can leave the ship a lasting *piece of kit*, not only
    /// stats and goodwill — a salvage writ pulls a warp coil from a dead hull, a deep
    /// survey brings back alien drive tech. It drops into the salvage hold to be
    /// installed in drydock, mirroring an event's `grant_component`. `None` = none.
    #[serde(default)]
    pub grant_component: Option<String>,
    /// Line narrating the boon; empty = a generic line.
    #[serde(default)]
    pub log: String,
}

impl CompletionReward {
    /// True when the reward grants nothing (an ordinary charter with no legacy).
    pub fn is_none(&self) -> bool {
        self.subsystem_deltas.is_empty()
            && self.reputation_deltas.is_empty()
            && self.faction_approval_deltas.is_empty()
            && self.grant_component.is_none()
            && self.resource == ResourceDelta::default()
            && self.population == PopulationDelta::default()
    }
}

/// A charter's standing per-year toll (content-depth charters round 13). Each delta
/// is applied once per voyage-year while the contract is under way.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnualToll {
    #[serde(default)]
    pub resource: ResourceDelta,
    #[serde(default)]
    pub ship: ShipDelta,
    #[serde(default)]
    pub population: PopulationDelta,
}

impl AnnualToll {
    /// True when the toll is entirely zero (an ordinary route with no standing cost).
    pub fn is_none(&self) -> bool {
        self.resource == ResourceDelta::default()
            && self.ship == ShipDelta::default()
            && self.population == PopulationDelta::default()
    }
}

/// One scripted, time-fixed beat of a charter (content-depth charters round 9):
/// the named event is forced by id once the voyage has run `at_year` years.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledBeat {
    pub template_id: String,
    pub at_year: u32,
}
