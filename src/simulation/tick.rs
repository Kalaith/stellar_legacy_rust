//! The simulation tick (GDD §3 step 3, §5.1).
//!
//! Time advances only on explicit player action (Pillar 4). `advance` steps the
//! month clock forward by the current speed step, applying the W1-tuned economic
//! year on each year boundary (production, upkeep, wear, aging, contract
//! progress, market) and rolling for a dated event every month — hard-stopping
//! the instant a decision, completion, or extinction lands (W3). The economic
//! year itself lives in `tick/economy.rs`.

mod beats;
mod economy;
#[cfg(test)]
mod tests;

use crate::data::contracts::ContractPhase;
use crate::data::GameData;
use crate::simulation::contract::SuccessLevel;
use crate::simulation::{contract, event_resolver, mortality, ship, subsystems};
use crate::state::sim::SimState;

use beats::*;

/// to: log lines are already recorded on the sim; a completed contract and a
/// blocking event are surfaced explicitly.
#[derive(Debug, Default)]
pub struct TickReport {
    /// Set when the active contract reached its target duration this year.
    pub contract_completed: Option<(f32, SuccessLevel)>,
    /// Set when an event fired that needs a council decision (not delegated).
    pub decision_required: bool,
    pub dynasty_extinct: bool,
    /// Set the month a *sitting leader dies in office* (real-time loop follow-up:
    /// mortality is continuous now). Reset at the top of each month; read by
    /// `fire_succession_beat` to force the ship to reckon with an untried command.
    pub leader_died: bool,
    /// Months actually advanced by this call before it stopped (W3). Less than
    /// the speed step's span whenever a decision hard-stops the advance early.
    pub months_advanced: u32,
    /// Set when the active contract crossed into a new authored phase this call
    /// (W2) — a hard-stop for the fast-forward, like a decision.
    pub phase_changed: Option<ContractPhase>,
}

/// Advance time up to `max_months`. Steps month by month, applying the W1-tuned
/// economic year on each year boundary and rolling for events every month, and
/// hard-stops the instant a council decision, contract completion, or extinction
/// lands — so an advance never skips past a moment that needs the player. The
/// real-time driver calls this with the whole months its accumulator crossed
/// (usually 1); tests/tooling pass a fixed span.
pub fn advance_months(sim: &mut SimState, data: &GameData, max_months: u32) -> TickReport {
    debug_assert!(
        !sim.has_pending_decision(),
        "caller must resolve the pending event/dilemma before advancing time"
    );
    let mut report = TickReport::default();

    for _ in 0..max_months {
        sim.month_clock += 1;
        report.months_advanced += 1;

        // The economic tick applies whole, on the year boundary — the W1 math
        // is untouched; only its cadence is now driven by the month clock.
        if sim.month_clock.is_multiple_of(12) {
            economy::year_boundary_tick(sim, data, &mut report);
        }

        // Monthly contract progress (W2): objective accrual on-station, the
        // authored phase timeline, milestones, and completion all step here.
        month_of_contract(sim, data, &mut report);

        // Monthly death roll (real-time loop follow-up): every living character
        // faces an age-scaled chance of death, and a vacated seat is filled. Sets
        // `dynasty_extinct` (loop hard-stops below) and `leader_died` (a beat).
        // `leader_died` is per-month, so clear last month's before the roll.
        report.leader_died = false;
        mortality::monthly_tick(sim, data, &mut report);

        // Monthly event step (GDD §5.4), dated to this exact month. Skipped on a
        // month that already produced a blocking dilemma, a completion, or an
        // extinction — one decision at a time, never piled onto a finished year.
        // A due campaign beat (W6) replaces the random roll; otherwise the
        // reactive/filler roll runs.
        if sim.pending_dilemma.is_none()
            && report.contract_completed.is_none()
            && !report.dynasty_extinct
            && !fire_succession_beat(sim, data, &mut report)
            && !fire_long_reign_beat(sim, data, &mut report)
            && !fire_dynasty_crisis_beat(sim, data, &mut report)
            && !fire_scheduled_beat(sim, data, &mut report)
            && !fire_charter_scheduled_beat(sim, data, &mut report)
            && !fire_due_beat(sim, data, &mut report)
            && !fire_drift_beat(sim, data, &mut report)
            && !fire_adaptation_beat(sim, data, &mut report)
            && !fire_crisis_beat(sim, data, &mut report)
            && !fire_loyalty_beat(sim, data, &mut report)
            && !fire_stability_beat(sim, data, &mut report)
            && !fire_despair_beat(sim, data, &mut report)
            && !fire_subsystem_beat(sim, data, &mut report)
            && !fire_hull_beat(sim, data, &mut report)
            && !fire_air_beat(sim, data, &mut report)
            && !fire_becalmed_beat(sim, data, &mut report)
            && !fire_divergence_beat(sim, data, &mut report)
            && !fire_cultural_divergence_beat(sim, data, &mut report)
            && !fire_reputation_beat(sim, data, &mut report)
            && !fire_recovery_beat(sim, data, &mut report)
            && !fire_stability_recovery_beat(sim, data, &mut report)
            && !fire_heartening_recovery_beat(sim, data, &mut report)
            && !fire_loyalty_recovery_beat(sim, data, &mut report)
            && !fire_hull_recovery_beat(sim, data, &mut report)
            && !fire_air_recovery_beat(sim, data, &mut report)
            && !fire_becalmed_recovery_beat(sim, data, &mut report)
            && !fire_flourish_beat(sim, data, &mut report)
            && !fire_depopulation_beat(sim, data, &mut report)
            && !fire_objective_beat(sim, data, &mut report)
            && !fire_founding_beat(sim, data, &mut report)
            && !fire_midvoyage_beat(sim, data, &mut report)
            && !fire_homecoming_beat(sim, data, &mut report)
            && !fire_power_transition_beat(sim, data, &mut report)
            && !fire_anniversary_beat(sim, data, &mut report)
            && !fire_dead_air_beat(sim, data, &mut report)
        {
            roll_monthly_event(sim, data, &mut report);
        }

        // Hard-stop the fast-forward the instant something needs attention — a
        // decision, a completion, an extinction, or crossing a phase boundary.
        if report.decision_required
            || report.contract_completed.is_some()
            || report.dynasty_extinct
            || report.phase_changed.is_some()
        {
            break;
        }
    }

    // Keep faction shares matched to the (possibly changed) head count (W7);
    // a faction rescaled to nothing is gone for good.
    for id in sim.rebalance_factions() {
        let name = crate::state::sim::factions::log_name(&data.factions, &id);
        sim.push_log(format!("The last of {name} is gone."));
    }

    sim.trim_log(data.config.log_limit);
    report
}

/// Test/tooling helper: advance up to one year's worth of the loop. Still
/// hard-stops early on a decision/completion/phase change, exactly like the
/// real-time driver would as the months tick past.
pub fn advance_year(sim: &mut SimState, data: &GameData) -> TickReport {
    advance_months(sim, data, 12)
}

/// One month of contract progress (W2): objective accrual on-station, the
/// authored phase timeline, milestone payouts, and completion detection. Logs
/// milestones and phase crossings; surfaces a phase change and completion on
/// the report so the fast-forward can hard-stop.
fn month_of_contract(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    // Fuel is only spent while under way toward the destination (W4): the phase
    // the month about to be processed falls in tells us whether we're burning.
    let travel_this_month = {
        let Some(contract) = sim.contract.as_ref() else {
            return;
        };
        contract.phase_at(contract.months_elapsed + 1).1 == ContractPhase::Travel
    };

    if travel_this_month {
        // A degraded engineering bay burns rich (content-depth subsystems round 20):
        // the base travel burn is scaled up as the drive's tuning slips.
        let burn = data.config.provisioning.fuel_burn_per_travel_month
            * subsystems::engineering_fuel_burn_factor(sim, data);
        if sim.ship.fuel < burn {
            // A dry tank in transit: the ship coasts. No progress toward the
            // destination this month (the voyage stretches), and this year's
            // systems decay will double — "the ship may not reach its
            // destination" (W4).
            sim.ship.fuel = 0.0;
            sim.stalled_months = sim.stalled_months.saturating_add(1);
            sim.fuel_stalled_this_year = true;
            return;
        }
        sim.ship.fuel = (sim.ship.fuel - burn).max(0.0);
    }

    let loadout = ship::loadout_stats(sim, data);
    let progress = contract::advance_contract(
        sim,
        &data.config,
        loadout.speed,
        loadout.combat,
        loadout.cargo,
        loadout.crew_capacity,
    );
    for milestone in &progress.reached_milestones {
        // Pooled so a voyage's many milestones don't read as a form letter (voice
        // round 19); indexed by log length so consecutive marks vary.
        let pool = &data.config.flavor.milestone;
        let line = if pool.is_empty() {
            format!("Milestone reached: {milestone}")
        } else {
            pool[sim.log.len() % pool.len()].replace("{milestone}", milestone)
        };
        sim.push_log(line);
    }
    if let Some(phase) = progress.phase_changed {
        let occurrence = sim
            .contract
            .as_ref()
            .map_or(1, |c| c.phase_occurrence(phase));
        sim.push_log(contract::phase_transition_line(
            &data.config.flavor,
            phase,
            occurrence,
        ));
        report.phase_changed = Some(phase);
    }
    if let Some(result) = progress.completed {
        report.contract_completed = Some(result);
    }
}

fn roll_monthly_event(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    if let Some(pending) = event_resolver::roll_event(sim, data) {
        apply_pending_event(sim, data, pending, report);
    }
}

/// Surface a rolled event: block for a council decision, or auto-resolve it
/// (delegated / no-decision), logging either way.
pub(crate) fn apply_pending_event(
    sim: &mut SimState,
    data: &GameData,
    pending: crate::state::sim::PendingEvent,
    report: &mut TickReport,
) {
    if let Some(template) = data.events.get(&pending.template_id).cloned() {
        let delegated = sim.delegation.is_delegated(template.category);
        if template.requires_decision && !delegated {
            // Pooled — this precedes every blocking decision, dozens a voyage, so a
            // flat prefix was the loudest repetition tell (voice round 19).
            let pool = &data.config.flavor.council_summons;
            let line = if pool.is_empty() {
                format!("Council decision required: {}", template.title)
            } else {
                pool[sim.log.len() % pool.len()].replace("{title}", &template.title)
            };
            sim.push_log(line);
            sim.pending_event = Some(pending);
            report.decision_required = true;
        } else {
            let label = event_resolver::auto_resolve(sim, data, &template);
            if delegated {
                sim.push_log(format!(
                    "Delegated advisor resolved '{}' with: {label}",
                    template.title
                ));
            }
        }
    }
}
