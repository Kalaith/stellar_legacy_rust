//! The simulation tick (GDD §3 step 3, §5.1).
//!
//! Time advances only on explicit player action (Pillar 4). `advance` steps the
//! month clock forward by the current speed step, applying the W1-tuned economic
//! year on each year boundary (production, upkeep, wear, aging, contract
//! progress, market) and rolling for a dated event every month — hard-stopping
//! the instant a decision, completion, or extinction lands (W3). The economic
//! year itself lives in `tick/economy.rs`.

mod economy;
#[cfg(test)]
mod tests;

use crate::data::contracts::ContractPhase;
use crate::data::GameData;
use crate::simulation::contract::SuccessLevel;
use crate::simulation::{contract, event_resolver, ship};
use crate::state::sim::{SimState, SpeedStep};

/// Everything a single year produced that the caller (game.rs) must react
/// to: log lines are already recorded on the sim; a completed contract and a
/// blocking event are surfaced explicitly.
#[derive(Debug, Default)]
pub struct TickReport {
    /// Set when the active contract reached its target duration this year.
    pub contract_completed: Option<(f32, SuccessLevel)>,
    /// Set when an event fired that needs a council decision (not delegated).
    pub decision_required: bool,
    pub dynasty_extinct: bool,
    /// Months actually advanced by this call before it stopped (W3). Less than
    /// the speed step's span whenever a decision hard-stops the advance early.
    pub months_advanced: u32,
    /// Set when the active contract crossed into a new authored phase this call
    /// (W2) — a hard-stop for the fast-forward, like a decision.
    pub phase_changed: Option<ContractPhase>,
}

/// Advance time by the current speed step (W3). Steps month by month, applying
/// the W1-tuned economic year on each year boundary and rolling for events every
/// month, and hard-stops the instant a council decision, contract completion, or
/// extinction lands — so a single Advance press never skips past a moment that
/// needs the player (Pillar 4).
pub fn advance(sim: &mut SimState, data: &GameData) -> TickReport {
    debug_assert!(
        !sim.has_pending_decision(),
        "caller must resolve the pending event/dilemma before advancing time"
    );
    let mut report = TickReport::default();

    for _ in 0..sim.speed.months() {
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

        // Monthly event step (GDD §5.4), dated to this exact month. Skipped on a
        // month that already produced a blocking dilemma, a completion, or an
        // extinction — one decision at a time, never piled onto a finished year.
        // A due campaign beat (W6) replaces the random roll; otherwise the
        // reactive/filler roll runs.
        if sim.pending_dilemma.is_none()
            && report.contract_completed.is_none()
            && !report.dynasty_extinct
            && !fire_scheduled_beat(sim, data, &mut report)
            && !fire_charter_scheduled_beat(sim, data, &mut report)
            && !fire_due_beat(sim, data, &mut report)
            && !fire_drift_beat(sim, data, &mut report)
            && !fire_adaptation_beat(sim, data, &mut report)
            && !fire_crisis_beat(sim, data, &mut report)
            && !fire_flourish_beat(sim, data, &mut report)
            && !fire_objective_beat(sim, data, &mut report)
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

/// Test/tooling helper: advance one year's worth of the loop at `OneYear` speed.
/// Still hard-stops early on a decision, exactly like a player pressing Advance
/// at 1-yr speed.
pub fn advance_year(sim: &mut SimState, data: &GameData) -> TickReport {
    sim.speed = SpeedStep::OneYear;
    advance(sim, data)
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
        let burn = data.config.provisioning.fuel_burn_per_travel_month;
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

    let speed = ship::loadout_stats(sim, data).speed;
    let progress = contract::advance_contract(sim, &data.config, speed);
    for milestone in &progress.reached_milestones {
        sim.push_log(format!("Milestone reached: {milestone}"));
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

/// Fire a due campaign beat (W6): if an unfired beat has come due this month,
/// mark it and force an event from its family (falling through to a normal roll
/// when the family is over-gated). Returns whether a beat replaced this month's
/// random roll.
fn fire_due_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let due = sim.contract.as_ref().and_then(|c| {
        c.beats
            .iter()
            .position(|b| !b.fired && b.month_clock <= sim.month_clock)
    });
    let Some(idx) = due else {
        return false;
    };
    let family = {
        let contract = sim.contract.as_mut().expect("beat came from the contract");
        contract.beats[idx].fired = true;
        contract.beats[idx].family.clone()
    };
    // A beat draws from its family (plus gates); if that leaves nothing, fall
    // through to the reactive roll so a beat never crashes or stalls.
    let pending = event_resolver::roll_event_in_family(sim, data, &family)
        .or_else(|| event_resolver::roll_event(sim, data));
    if let Some(pending) = pending {
        apply_pending_event(sim, data, pending, report);
    }
    true
}

/// Fire a cultural-drift threshold beat (content-depth round 2): the first month
/// the people's `cultural_drift` reaches the next authored threshold, force a beat
/// from the drift family so the Long-Term Expedition beats read as consequences
/// of how far the voyage has changed the crew. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
fn fire_drift_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.drift_beats_fired as usize) < cfg.drift_beats.len()
            && sim.population.cultural_drift >= cfg.drift_beats[c.drift_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.drift_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.drift_beat_family, report);
    true
}

/// Fire an adaptation threshold beat (content-depth round 3): the physiological
/// parallel to `fire_drift_beat`. As the people's `adaptation` crosses each
/// authored threshold, force a beat from the adaptation family — the descendants
/// growing suited to the ship in body and instinct.
fn fire_adaptation_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.adaptation_beats_fired as usize) < cfg.adaptation_beats.len()
            && sim.population.adaptation >= cfg.adaptation_beats[c.adaptation_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.adaptation_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.adaptation_beat_family, report);
    true
}

/// Fire a cohesion-collapse crisis beat (content-depth round 6): the *descending*
/// mirror of the drift/adaptation beats. As the people's `unity` falls to or
/// below each authored threshold (high→low), force a beat from the crisis family
/// — a fracturing ship generates its own reckoning rather than waiting on a
/// random roll. Fires at most one threshold per month; returns whether it
/// replaced the reactive roll.
fn fire_crisis_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.crisis_beats_fired as usize) < cfg.crisis_beats.len()
            && sim.population.unity <= cfg.crisis_beats[c.crisis_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.crisis_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.crisis_beat_family, report);
    true
}

/// Fire a flourish beat (content-depth round 8): the *ascending* positive pole of
/// the crisis beat. The first month the people's `morale` climbs to or past each
/// authored threshold (low→high), force a beat from the flourish family — a
/// thriving, well-stewarded ship surfaces its own golden age instead of the
/// skeleton only ever answering to trouble. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
fn fire_flourish_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.flourish_beats_fired as usize) < cfg.flourish_beats.len()
            && sim.population.morale >= cfg.flourish_beats[c.flourish_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.flourish_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.flourish_beat_family, report);
    true
}

/// Fire an objective-progress beat (content-depth round 9): the first pacing
/// keyed to the mission itself. As the active charter's objective crosses each
/// authored fraction (low→high) a beat is forced — the crew marking a purpose
/// most of them will not live to see completed. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
fn fire_objective_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.objective_beats_fired as usize) < cfg.objective_beats.len()
            && c.objective_fraction() >= cfg.objective_beats[c.objective_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.objective_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.objective_beat_family, report);
    true
}

/// Fire an anniversary beat (content-depth round 7): a *periodic* archetype, not
/// a threshold one — every `anniversary_years` of the voyage a beat is forced
/// from the anniversary family, giving the crossing a commemorative heartbeat as
/// the founding recedes into ritual. Fires at most one anniversary per month;
/// returns whether it replaced the reactive roll.
fn fire_anniversary_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.anniversary_years == 0 {
        return false;
    }
    let due = sim.contract.as_ref().is_some_and(|c| {
        let next_month = (c.anniversaries_fired + 1) * cfg.anniversary_years * 12;
        sim.month_clock >= next_month
    });
    if !due {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.anniversaries_fired += 1;
    }
    force_family_beat(sim, data, &cfg.anniversary_beat_family, report);
    true
}

/// Fire a dead-air backstop beat (content-depth round 5): once more than
/// `dead_air_years` have passed with no event, guarantee one rather than let the
/// voyage drift on empty — long eventless stretches are a coverage bug, not a
/// mercy. The family is drawn from `dead_air_pool` via the state RNG (so a seed
/// still replays), and forcing a beat resets the event clock. Only while a
/// contract is under way; off when `dead_air_years` is 0. Returns whether it
/// replaced this month's reactive roll.
fn fire_dead_air_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.dead_air_years == 0 || cfg.dead_air_pool.is_empty() || sim.contract.is_none() {
        return false;
    }
    let gap_months = sim.month_clock.saturating_sub(sim.last_event_month_clock);
    if gap_months < cfg.dead_air_years * 12 {
        return false;
    }
    let pick = sim.rng.below(cfg.dead_air_pool.len());
    let family = cfg.dead_air_pool[pick].clone();
    force_family_beat(sim, data, &family, report);
    // Reset the gap even if the pick found no candidate this month, so a genuinely
    // over-gated moment waits another full interval rather than retrying monthly.
    sim.last_event_month_clock = sim.month_clock;
    true
}

/// Force one event from `family` (falling through to a normal reactive roll when
/// the family is over-gated), applying it. Shared by the threshold-beat firers.
fn force_family_beat(sim: &mut SimState, data: &GameData, family: &str, report: &mut TickReport) {
    let pending = event_resolver::roll_event_in_family(sim, data, family)
        .or_else(|| event_resolver::roll_event(sim, data));
    if let Some(pending) = pending {
        apply_pending_event(sim, data, pending, report);
    }
}

/// Force a specific event by id (content-depth): build a pending event for it and
/// apply it, bypassing gates — the shared path for both the scheduled follow-up
/// (round 9) and a charter's scripted timed beats (charters round 9).
fn force_event_beat(
    sim: &mut SimState,
    data: &GameData,
    template_id: String,
    report: &mut TickReport,
) {
    apply_pending_event(
        sim,
        data,
        crate::state::sim::PendingEvent {
            template_id,
            rolled_month_clock: sim.month_clock,
        },
        report,
    );
    sim.last_event_month_clock = sim.month_clock;
}

/// Fire a charter's scripted timed beat (content-depth charters round 9): a
/// mission built around a reckoning on a known clock forces its next beat once
/// this voyage has run to its `at_year`. Beats are authored ascending and fire in
/// order, one per month; returns whether it replaced the reactive roll.
fn fire_charter_scheduled_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let due = sim.contract.as_ref().and_then(|c| {
        let years = c.months_elapsed / 12;
        c.scheduled_beats
            .get(c.scheduled_beats_fired as usize)
            .filter(|b| years >= b.at_year)
            .map(|b| b.template_id.clone())
    });
    let Some(template_id) = due else {
        return false;
    };
    if let Some(c) = sim.contract.as_mut() {
        c.scheduled_beats_fired += 1;
    }
    force_event_beat(sim, data, template_id, report);
    true
}

/// Fire a scheduled follow-up (content-depth event families round 9): the timed,
/// deterministic payoff of an outcome's `schedule_followup`. Once the voyage
/// reaches the earliest due `fire_year`, that named event is forced by id — past
/// its gates, since a scheduled-only payoff never rolls — so an authored arc lands
/// on its promised clock. Fires at most one per month (earliest year first, ties
/// broken by id for determinism); returns whether it replaced the reactive roll.
fn fire_scheduled_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let year = sim.year();
    let due = sim
        .scheduled_events
        .iter()
        .enumerate()
        .filter(|(_, s)| s.fire_year <= year)
        .min_by(|(_, a), (_, b)| {
            a.fire_year
                .cmp(&b.fire_year)
                .then_with(|| a.template_id.cmp(&b.template_id))
        })
        .map(|(i, _)| i);
    let Some(idx) = due else {
        return false;
    };
    let scheduled = sim.scheduled_events.remove(idx);
    force_event_beat(sim, data, scheduled.template_id, report);
    true
}

/// Surface a rolled event: block for a council decision, or auto-resolve it
/// (delegated / no-decision), logging either way.
fn apply_pending_event(
    sim: &mut SimState,
    data: &GameData,
    pending: crate::state::sim::PendingEvent,
    report: &mut TickReport,
) {
    if let Some(template) = data.events.get(&pending.template_id).cloned() {
        let delegated = sim.delegation.is_delegated(template.category);
        if template.requires_decision && !delegated {
            sim.push_log(format!("Council decision required: {}", template.title));
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
