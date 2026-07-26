//! Contract success scoring and progression (GDD §5.2).

use crate::data::contracts::{ContractPhase, ContractTemplate, MetricKind};
use crate::data::ResourceDelta;
use crate::state::sim::{ActiveContract, MetricState, MilestoneState, SimState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessLevel {
    Complete,
    Partial,
    Pyrrhic,
    Failure,
}

impl SuccessLevel {
    pub fn label(self) -> &'static str {
        match self {
            SuccessLevel::Complete => "Complete",
            SuccessLevel::Partial => "Partial",
            SuccessLevel::Pyrrhic => "Pyrrhic",
            SuccessLevel::Failure => "Failure",
        }
    }
}

/// `success_score = Σ( min(1, current/target) * weight )`, banded per GDD §5.2.
pub fn score_success(metrics: &[MetricState]) -> (f32, SuccessLevel) {
    let score: f32 = metrics
        .iter()
        .map(|m| {
            let ratio = if m.target <= 0.0 {
                1.0
            } else {
                (m.current / m.target).min(1.0)
            };
            ratio * m.weight
        })
        .sum();

    let level = if score >= 0.9 {
        SuccessLevel::Complete
    } else if score >= 0.7 {
        SuccessLevel::Partial
    } else if score >= 0.4 {
        SuccessLevel::Pyrrhic
    } else {
        SuccessLevel::Failure
    };
    (score, level)
}

/// Everything one month of contract time produced that the tick must surface.
#[derive(Debug, Default)]
pub struct ContractProgress {
    pub reached_milestones: Vec<String>,
    /// Set when this month crossed into a new authored phase (W2).
    pub phase_changed: Option<ContractPhase>,
    /// Set the month the contract reaches its full duration.
    pub completed: Option<(f32, SuccessLevel)>,
}

/// Whether a charter's in-world availability gate is met right now (content-depth
/// charters round 12): every people it names is aboard. The charter-level parallel
/// to the outcome gates — checked by both the writ board (to lock/label it) and the
/// select action (so a locked writ can't be put under consideration). `min_renown`
/// stays a separate, cross-campaign gate; this reads the living roster.
pub fn meets_in_world_gate(sim: &SimState, template: &ContractTemplate) -> bool {
    template
        .requires_faction_aboard
        .iter()
        .all(|id| sim.is_faction_aboard(id))
        // Deed gates (content-depth charters round 14): a writ can require the ship
        // to have *done* something (how a charter arc unlocks its next leg) or be
        // barred by a dark deed on record.
        && template
            .requires_consequence
            .iter()
            .all(|tag| sim.consequences.contains(tag))
        && !template
            .forbidden_consequence
            .iter()
            .any(|tag| sim.consequences.contains(tag))
        // Reputation gates (content-depth charters round 16): the writ board reflects
        // the ship's cumulative character — a merciful name opens some work, a feared
        // one others.
        && template
            .min_reputation
            .iter()
            .all(|g| sim.reputation(&g.id) >= g.threshold)
        && template
            .max_reputation
            .iter()
            .all(|g| sim.reputation(&g.id) <= g.threshold)
}

/// Whether the ship's *loadout* meets a charter's minimum fitting (content-depth
/// charters round 26): the drydock-availability twin of `meets_in_world_gate`. A writ
/// that names a `min_combat`/`min_cargo`/`min_speed` is offered only to a hull whose
/// aggregate loadout clears each — the guns to serve an enforcement contract, the hold
/// to carry a megahaul, the engine to make a deep window. Kept a *separate* gate from
/// the in-world one so the writ board can name which of the two bars a locked writ.
/// A charter that names no minimum (the founding-tier default) always clears.
pub fn meets_loadout_gate(
    sim: &SimState,
    data: &crate::data::GameData,
    template: &ContractTemplate,
) -> bool {
    if template.min_combat == 0 && template.min_cargo == 0 && template.min_speed == 0 {
        return true;
    }
    let loadout = crate::simulation::ship::loadout_stats(sim, data);
    loadout.combat >= template.min_combat
        && loadout.cargo >= template.min_cargo
        && loadout.speed >= template.min_speed
}

/// Grant a charter's completion reward (content-depth charters round 15): the
/// lasting capability a mission leaves the ship — chiefly subsystem boons kept
/// across voyages — applied once when the charter is seen through to full term.
/// `performance` is the mission's success score (content-depth charters round 25):
/// the *lessons* a mission leaves — the subsystem capability boon — scale with how
/// *well* it was done, so a barely-scraped Pyrrhic completion teaches the crew far less
/// than a clean one, closing the loop with the it accrual accelerators (speed/combat/
/// cargo/morale) that help a ship finish *better*. The deed and the recovery it earns
/// (reputation, faction goodwill, a salvaged component) are binary — they happened, or
/// they didn't — so those apply at full. Returns the narration line (empty if the reward
/// is empty). No-op for an ordinary charter.
pub fn apply_completion_reward(
    sim: &mut SimState,
    template: &ContractTemplate,
    performance: f32,
) -> Option<String> {
    let reward = &template.completion_reward;
    if reward.is_none() {
        return None;
    }
    let scale = performance.clamp(0.0, 1.0);
    sim.resources.apply(&reward.resource);
    sim.population.apply(&reward.population);
    // The capability *lessons* scale with how well the mission went (round 25); a
    // scrappy completion leaves the crew a fainter version of the craft a clean one would.
    for delta in &reward.subsystem_deltas {
        if let Some(state) = sim.subsystems.get_mut(&delta.id) {
            state.condition = (state.condition + delta.condition * scale).clamp(0.0, 1.0);
            state.knowledge = (state.knowledge + delta.knowledge * scale).clamp(0.0, 1.0);
        }
    }
    // A whole voyage of one kind of work shapes the ship's character (content-depth
    // charters round 17): the mission the reputation unlocked now builds it further.
    for delta in &reward.reputation_deltas {
        sim.adjust_reputation(&delta.id, delta.delta);
    }
    // …and earns the goodwill of the peoples it served (content-depth charters round
    // 19): a completed mission can leave the named aboard factions delighted, feeding
    // the round-19 gift beats. Factions not aboard are ignored.
    for delta in &reward.faction_approval_deltas {
        if let Some(state) = sim
            .factions
            .iter_mut()
            .find(|f| f.faction_id == delta.id && f.is_aboard())
        {
            state.adjust_approval(delta.delta);
        }
    }
    // …and a mission can leave the ship a lasting piece of kit (content-depth
    // charters round 20): the recovered component drops into the salvage hold, to be
    // installed in drydock — mirroring an event's `grant_component`.
    if let Some(component_id) = &reward.grant_component {
        sim.ship.salvage.push(component_id.clone());
    }
    // …and a subsystem version the charter unlocks (2c): a mission-reward fitting,
    // buildable in drydock once the ship has earned it.
    if let Some(fitting_id) = &reward.grant_fitting {
        if !sim.ship.unlocked_fittings.contains(fitting_id) {
            sim.ship.unlocked_fittings.push(fitting_id.clone());
        }
    }
    Some(if reward.log.is_empty() {
        format!("The lessons of {} stay with the ship.", template.name)
    } else {
        reward.log.clone()
    })
}

/// Mark the ship's name for a charter it did not see through (content-depth charters
/// round 18): the negative mirror of `apply_completion_reward`, applied once when a
/// charter concludes at Failure. A defaulted or abandoned mission costs the ship's
/// *character* — a hardened mercy for a relief run given up, a name for folding
/// (`resolve`) for any writ quit half-done. Returns the narration line (empty for a
/// charter whose failure marks nothing). No-op for an ordinary charter.
pub fn apply_abandonment(sim: &mut SimState, template: &ContractTemplate) -> Option<String> {
    let ab = &template.abandonment;
    if ab.is_none() {
        return None;
    }
    for delta in &ab.reputation_deltas {
        sim.adjust_reputation(&delta.id, delta.delta);
    }
    Some(if ab.log.is_empty() {
        format!(
            "Word travels the dark that the ship gave up the {}.",
            template.name
        )
    } else {
        ab.log.clone()
    })
}

/// Instantiate an active contract from a template at the current sim state.
pub fn start_contract(template: &ContractTemplate, sim: &SimState) -> ActiveContract {
    ActiveContract {
        template_id: template.id.clone(),
        name: template.name.clone(),
        objective: template.objective,
        target_duration_years: template.target_duration_years,
        months_elapsed: 0,
        phase: ContractPhase::Preparation,
        phases: template.phases.clone(),
        phase_index: 0,
        metrics: template
            .success_metrics
            .iter()
            .map(|m| MetricState {
                id: m.id.clone(),
                kind: m.kind,
                name: m.name.clone(),
                weight: m.weight,
                target: m.target,
                current: 0.0,
            })
            .collect(),
        milestones: template
            .milestones
            .iter()
            .map(|m| MilestoneState {
                id: m.id.clone(),
                name: m.name.clone(),
                progress_threshold: m.progress_threshold,
                reached: false,
                reward: m.reward,
            })
            .collect(),
        starting_population: sim.population.count,
        objective_target: template.objective_target,
        objective_unit: template.objective_unit.clone(),
        // A preserve charter (round 23) sets out *carrying* the full objective and only
        // loses it; an ordinary one builds from zero.
        objective_progress: if template.preserve_objective {
            template.objective_target
        } else {
            0.0
        },
        // Beats are laid out at LAUNCH by the caller (W6); a bare contract has
        // none until then.
        beats: Vec::new(),
        healthy_food_months: 0,
        healthy_energy_months: 0,
        tags: template.tags.clone(),
        beat_families: template.beat_families.clone(),
        drift_beats_fired: 0,
        adaptation_beats_fired: 0,
        crisis_beats_fired: 0,
        loyalty_beats_fired: 0,
        stability_beats_fired: 0,
        despair_beats_fired: 0,
        hull_beats_fired: 0,
        air_beats_fired: 0,
        becalmed_beats_fired: 0,
        anniversaries_fired: 0,
        flourish_beats_fired: 0,
        objective_beats_fired: 0,
        homecoming_beat_fired: false,
        midvoyage_beat_fired: false,
        hazard: template.hazard,
        scheduled_beats: template.scheduled_beats.clone(),
        scheduled_beats_fired: 0,
        objective_subsystem: template.objective_subsystem.clone(),
        objective_combat_scaling: template.objective_combat_scaling,
        objective_cargo_scaling: template.objective_cargo_scaling,
        preserve_objective: template.preserve_objective,
        preserve_attrition_per_year: template.preserve_attrition_per_year,
    }
}

/// Advance the active contract by one month (W2): step the timeline, recompute
/// the authored phase, accrue objective work while on-station, refresh metrics,
/// pay any newly reached milestone, and detect completion. `speed`, `combat`,
/// `cargo`, and `crew` are the ship loadout's aggregate stats: speed quickens *every*
/// mission's objective, combat quickens only a *contested* one (charters round 21), cargo
/// only a *haul* one (charters round 24), and crew_capacity (berths) eases the attrition of a
/// *preserve* one (charters round 28) — a ship with room to carry keeps more of what it carries.
pub fn advance_contract(
    sim: &mut SimState,
    config: &crate::data::GameConfig,
    speed: i32,
    combat: i32,
    cargo: i32,
    crew: i32,
) -> ContractProgress {
    let population_count = sim.population.count;
    let unity = sim.population.unity;
    let food_ok = sim.resources.food >= config.low_food_threshold;
    let energy_ok = sim.resources.energy >= config.low_energy_threshold;
    let progress_per_speed = config.ship.contract_progress_per_speed;

    // The module this mission leans on scales how fast its work accrues (content-depth
    // subsystems round 14): a pristine bay works at the base rate, a degraded one
    // slower. Read before the mutable contract borrow. Penalty-below-full keeps the
    // baseline, so a well-kept ship's objective is unchanged.
    let objective_condition = sim
        .contract
        .as_ref()
        .filter(|c| !c.objective_subsystem.is_empty())
        .map(|c| {
            let cond = sim
                .subsystems
                .get(&c.objective_subsystem)
                .map_or(1.0, |s| s.condition);
            (1.0 - config.subsystems.objective_condition_penalty * (1.0 - cond)).max(0.0)
        })
        .unwrap_or(1.0);

    // A crew's spirits move how fast the work goes (content-depth charters round 22):
    // the objective's first coupling to the crew's *state* rather than the ship's
    // fittings (speed, combat) or its modules. A devoted, high-hearted crew drives the
    // work harder than any drive tuning; a dispirited one drags at it. A swing around
    // the neutral midpoint, floored so a broken crew slows the mission but never wholly
    // stalls it. Read before the mutable contract borrow.
    let morale_factor =
        (1.0 + config.ship.morale_objective_swing * (sim.population.morale - 0.5)).max(0.2);
    // …and its cohesion moves how *together* the work goes (content-depth charters round 34): the
    // second crew-state lever, coordination beside morale's will. A united crew works as one hand, a
    // fractured one duplicates effort and argues the method — scaled around the neutral midpoint and
    // floored like morale, multiplying with it so a mission goes fastest under a crew both willing
    // and united. Read before the mutable contract borrow.
    let unity_factor =
        (1.0 + config.ship.unity_objective_swing * (sim.population.unity - 0.5)).max(0.2);

    let mut out = ContractProgress::default();
    // The objective subsystem an Operation month trained (content-depth charters round 33), set
    // inside the contract-borrow scope and applied after it ends so we can mutate `sim.subsystems`.
    let mut trained_subsystem: Option<String> = None;

    // Mutate the contract in a scope so its borrow ends before we grant any
    // milestone rewards to the shared resource pool.
    let rewards = {
        let Some(contract) = sim.contract.as_mut() else {
            return out;
        };

        let prev_phase = contract.phase;
        contract.months_elapsed += 1;
        // Provisioning discipline accrues month by month: each upkeep store
        // above its crisis threshold banks credit toward ResourceEfficiency.
        contract.healthy_food_months += food_ok as u32;
        contract.healthy_energy_months += energy_ok as u32;
        let (index, phase) = contract.phase_at(contract.months_elapsed);
        contract.phase_index = index;
        contract.phase = phase;
        if phase != prev_phase {
            out.phase_changed = Some(phase);
        }

        // A preserve charter (round 23) does not *build* its objective — it carries it,
        // and loses a little every month of the voyage (the cold banks fail, the sick do
        // not all wake). Applied across Travel/Operation/Return, not the pre-launch or
        // post-return bookends; hazard events take the rest. No accrual.
        if contract.preserve_objective {
            if matches!(
                phase,
                ContractPhase::Travel | ContractPhase::Operation | ContractPhase::Return
            ) {
                // Berths ease the attrition (content-depth charters round 28): a ship with the
                // crew_capacity to carry its charge in some comfort loses fewer of them over the
                // long dark than one that crams them into every hold — crew_capacity's first
                // mechanical role, the berth twin of cargo's haul lever. Floored so even the
                // roomiest ship cannot wholly stop the loss; inert (factor 1.0) at crew 0.
                let berth_relief =
                    (1.0 - crew.max(0) as f32 * config.ship.preserve_berth_relief).max(0.2);
                let monthly_loss = contract.objective_target * contract.preserve_attrition_per_year
                    / 12.0
                    * berth_relief;
                contract.objective_progress = (contract.objective_progress - monthly_loss).max(0.0);
            }
        } else if phase == ContractPhase::Operation {
            // Objective work happens only on-station (Operation): base_rate spreads
            // the target across the operation window, and ship speed quickens it.
            let operation_months = contract.operation_months().max(1);
            let base_rate = contract.objective_target / operation_months as f32;
            let speed_factor = 1.0 + speed.max(0) as f32 * progress_per_speed;
            // A contested writ (round 21) is worked faster by an armed ship; a
            // mission that sets no combat scaling is indifferent to firepower, so
            // combat_factor is 1.0 and the accrual is unchanged there.
            let combat_factor = 1.0 + combat.max(0) as f32 * contract.objective_combat_scaling;
            // A haul writ (round 24) is worked faster by a bigger hold; a mission whose
            // objective is not a quantity of material sets no cargo scaling, so this is
            // 1.0 there and the accrual is unchanged.
            let cargo_factor = 1.0 + cargo.max(0) as f32 * contract.objective_cargo_scaling;
            contract.objective_progress += base_rate
                * speed_factor
                * objective_condition
                * combat_factor
                * cargo_factor
                * morale_factor
                * unity_factor;
            // …and the work itself sharpens the craft it leans on (content-depth charters round 33):
            // the reverse of the round-14 coupling, where the subsystem's condition speeds the
            // mission — here a month of on-station work builds the objective subsystem's *knowledge*
            // (a mining survey masters the engineering bay's craft, a greening its agriculture),
            // closing the loop. Captured here, applied after the contract borrow ends. Knowledge,
            // not condition, so it never feeds back into faster accrual (no runaway).
            if !contract.objective_subsystem.is_empty() {
                trained_subsystem = Some(contract.objective_subsystem.clone());
            }
        }

        let progress = contract.progress();
        let mut reached_rewards = Vec::new();
        for milestone in &mut contract.milestones {
            if !milestone.reached && progress >= milestone.progress_threshold {
                milestone.reached = true;
                out.reached_milestones.push(milestone.name.clone());
                reached_rewards.push(milestone.reward);
            }
        }

        let objective_fraction = contract.objective_fraction();
        let upkeep_health = contract.upkeep_health();
        for metric in &mut contract.metrics {
            metric.current = match metric.kind {
                MetricKind::PopulationSurvival => {
                    if contract.starting_population == 0 {
                        1.0
                    } else {
                        population_count as f32 / contract.starting_population as f32
                    }
                }
                // Mission completion now reads the quantified objective (W2).
                MetricKind::MissionCompletion => objective_fraction,
                // Provisioning discipline across the whole voyage: the fraction
                // of elapsed months each upkeep store held above its crisis
                // threshold. A ship that never ran low scores 1.0; every lean
                // month drags the score down for the rest of the contract.
                MetricKind::ResourceEfficiency => upkeep_health,
                MetricKind::SocialCohesion => unity,
            };
        }

        if contract.months_elapsed >= contract.total_months() {
            out.completed = Some(score_success(&contract.metrics));
        }

        reached_rewards
    };

    // A milestone's reward lands the month it is first reached.
    for reward in rewards {
        sim.resources.apply(&reward);
    }
    // The month's on-station work sharpens the objective subsystem's craft (content-depth charters
    // round 33): a small knowledge gain, applied now the contract borrow has ended. Inert when the
    // gain is 0 or the mission leans on no subsystem.
    if let Some(sub_id) = trained_subsystem {
        let gain = config.subsystems.objective_subsystem_training_per_month;
        if gain > 0.0 {
            if let Some(state) = sim.subsystems.get_mut(&sub_id) {
                state.knowledge = (state.knowledge + gain).min(1.0);
            }
        }
    }
    out
}

/// Turn the ship for home early (W2): jump the contract to the start of its
/// first Return segment, freezing objective progress where it stands. No-op
/// without a contract, without a Return segment, or already in/past Return.
/// Returns whether the ship turned back.
pub fn jump_to_return(sim: &mut SimState) -> bool {
    let Some(contract) = sim.contract.as_mut() else {
        return false;
    };
    let Some(return_index) = contract.first_return_index() else {
        return false;
    };
    let return_start = contract.segment_start(return_index);
    if contract.months_elapsed >= return_start {
        return false;
    }
    contract.months_elapsed = return_start;
    contract.phase_index = return_index;
    contract.phase = contract.phases[return_index].kind;
    true
}

/// The reputation premium (or discount) on a charter's pay (content-depth charters round 29):
/// where `min_reputation` gates *which* work a name unlocks (round 16), this scales how *well*
/// that work pays. The pay is multiplied by `1 + scale·(reputation − 0.5)` on the charter's
/// named trait, floored at 0.5 — so a ship famous for the trait earns a premium, a notorious one
/// a discount, and a neutral name the base terms. 1.0 (inert) when the charter names no trait or
/// sets a zero scale. Reads the ship's cumulative reputation; deterministic, no RNG.
pub fn reputation_reward_multiplier(sim: &SimState, template: &ContractTemplate) -> f32 {
    if template.reward_reputation_trait.is_empty() || template.reward_reputation_scale == 0.0 {
        return 1.0;
    }
    let rep = sim.reputation(&template.reward_reputation_trait);
    (1.0 + template.reward_reputation_scale * (rep - 0.5)).max(0.5)
}

/// The morale shift a concluded mission's *outcome* leaves the crew (content-depth charters round
/// 31): the crew's emotional stake in the ship's purpose, distinct from the pay (the treasury),
/// the reputation (the ship's name), and the faction goodwill (the peoples). A mission seen
/// through lifts spirits (pride in the work), one botched or abandoned dents them (the failure
/// felt) — `scale · (score − 0.5)`, centred on a middling result so a clean Complete lifts, a
/// Failure dents, and a break-even Pyrrhic barely moves the needle. Composes with the round-29
/// despair and round-30 heartening beats: a run of failures can drive the crew toward despair, a
/// string of wins lift them out. 0 (inert) when the scale is 0.
pub fn mission_outcome_morale_shift(score: f32, scale: f32) -> f32 {
    scale * (score - 0.5)
}

/// Prorate a charter reward by objective completion (W2): pay = reward ×
/// fraction, rounded toward zero per resource. Every completion pays exactly
/// this — full-term or truncated; zero objective progress ⇒ zero pay.
pub fn prorated_reward(reward: &ResourceDelta, fraction: f32) -> ResourceDelta {
    ResourceDelta {
        credits: (reward.credits as f32 * fraction) as i64,
        energy: (reward.energy as f32 * fraction) as i64,
        minerals: (reward.minerals as f32 * fraction) as i64,
        food: (reward.food as f32 * fraction) as i64,
        influence: (reward.influence as f32 * fraction) as i64,
    }
}

/// The log line narrating a crossing into `phase` (W2).
/// The log line for entering `phase` on its `occurrence`-th time this voyage
/// (1-based). Draws from the data-driven `flavor.phase_lines` pool so a
/// double-hop's second departure/arrival reads differently from the first
/// (content-depth voice round 3); an empty or missing pool falls back to the
/// built-in line so the log is never blank.
pub fn phase_transition_line(
    flavor: &crate::data::FlavorConfig,
    phase: ContractPhase,
    occurrence: usize,
) -> String {
    let key = match phase {
        ContractPhase::Preparation => "preparation",
        ContractPhase::Travel => "travel",
        ContractPhase::Operation => "operation",
        ContractPhase::Return => "return",
        ContractPhase::Completion => "completion",
    };
    if let Some(pool) = flavor.phase_lines.get(key) {
        if !pool.is_empty() {
            return pool[occurrence.saturating_sub(1) % pool.len()].clone();
        }
    }
    match phase {
        ContractPhase::Preparation => "Standing by for departure.",
        ContractPhase::Travel => "Departure burn complete — the ship is underway.",
        ContractPhase::Operation => "The ship makes station. On-site operations begin.",
        ContractPhase::Return => "Objective work concluded — course set for home.",
        ContractPhase::Completion => "The ship returns to its home berth.",
    }
    .to_string()
}

#[cfg(test)]
mod tests;
