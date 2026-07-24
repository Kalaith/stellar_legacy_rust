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
use crate::simulation::{contract, event_resolver, mortality, ship, subsystems};
use crate::state::sim::SimState;

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
            && !fire_subsystem_beat(sim, data, &mut report)
            && !fire_hull_beat(sim, data, &mut report)
            && !fire_reputation_beat(sim, data, &mut report)
            && !fire_recovery_beat(sim, data, &mut report)
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

/// Fire a loyalty-collapse beat (content-depth round 14): the last identity stat
/// to get a beat. As `legacy_loyalty` falls to or below each authored threshold
/// (high→low), force a beat — not the cultural drift the drift beats mark but the
/// political one, the founders' covenant lapsing. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
fn fire_loyalty_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.loyalty_beats_fired as usize) < cfg.loyalty_beats.len()
            && sim.population.legacy_loyalty <= cfg.loyalty_beats[c.loyalty_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.loyalty_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.loyalty_beat_family, report);
    true
}

/// Fire a stability-collapse beat (content-depth round 15): the last population stat
/// to get a beat. As `stability` falls to or below each authored threshold (high→
/// low), force a beat — not the people fracturing (crisis) nor the founders' authority
/// lapsing (loyalty), but the ship's own institutions ceasing to function. Fires at
/// most one threshold per month; returns whether it replaced the reactive roll.
fn fire_stability_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.stability_beats_fired as usize) < cfg.stability_beats.len()
            && sim.population.stability <= cfg.stability_beats[c.stability_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.stability_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.stability_beat_family, report);
    true
}

/// Fire a subsystem-collapse beat (content-depth round 17): the first forced beat
/// keyed to a *subsystem's condition* — the physical-crisis dimension the beat
/// lattice never watched. The first tick a configured module's condition falls to or
/// below its red line, a beat is forced from its family (a keystone that has truly
/// failed is a defining voyage crisis, guaranteed a reckoning rather than left to a
/// reactive roll). Campaign-scoped, once per module a voyage. Fires only during a
/// voyage; at most one per month.
fn fire_subsystem_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.subsystem_beats.is_empty() || sim.contract.is_none() {
        return false;
    }
    let hit = cfg.subsystem_beats.iter().find(|b| {
        !sim.subsystem_beats_fired.contains(&b.subsystem)
            && sim
                .subsystems
                .get(&b.subsystem)
                .is_some_and(|s| s.condition <= b.threshold)
    });
    let Some(beat) = hit else {
        return false;
    };
    let family = beat.family.clone();
    sim.subsystem_beats_fired.push(beat.subsystem.clone());
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a hull-collapse beat (content-depth campaign-skeleton round 23): the structural
/// twin of the subsystem-collapse beat — where that watches a *module's* condition, this
/// watches the *ship's own frame*. The month `hull_integrity` first falls to or below the
/// red line, a beat is forced (the crew confronting that the vessel itself is failing);
/// a refit back above the line re-arms it, so a ship rebuilt and let fail again reckons
/// anew. Fires only during a voyage; at most one per crossing.
fn fire_hull_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.hull_beat_family.is_empty() || cfg.hull_beat_threshold <= 0.0 || sim.contract.is_none() {
        return false;
    }
    let band = if sim.ship.hull_integrity <= cfg.hull_beat_threshold {
        -1
    } else {
        0
    };
    if band == sim.hull_beat_band {
        return false;
    }
    sim.hull_beat_band = band;
    if band == 0 {
        // The hull recovered above the red line — re-arm, but do not fire.
        return false;
    }
    let family = cfg.hull_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a reputation beat (content-depth round 16): the skeleton's first trigger on
/// the ship's *cumulative character* (it105), not a population stat. When the named
/// reputation trait crosses *into* a strong band — famously high or notoriously low —
/// force a beat, the ship reckoning with the name it has earned; a return to the
/// middle silently re-arms it. Fires only during a voyage; at most one per month.
fn fire_reputation_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.reputation_beat_trait.is_empty()
        || cfg.reputation_beat_family.is_empty()
        || sim.contract.is_none()
    {
        return false;
    }
    let value = sim.reputation(&cfg.reputation_beat_trait);
    let band = if value >= cfg.reputation_beat_high {
        1
    } else if value <= cfg.reputation_beat_low {
        -1
    } else {
        0
    };
    if band == sim.reputation_beat_band {
        return false;
    }
    // A return to the middle re-arms silently; only crossing *into* a strong name fires.
    if band == 0 {
        sim.reputation_beat_band = 0;
        return false;
    }
    sim.reputation_beat_band = band;
    let family = cfg.reputation_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a recovery beat (content-depth round 13): the crisis beat's hopeful mirror.
/// Once the ship has fractured (a crisis beat fired) and its `unity` climbs back to
/// or above the recovery threshold, force a beat — the mending, a ship pulling back
/// from the brink — and reset the crisis counter so a relapse re-arms the collapse
/// beats. Fires once per crisis episode (the reset clears the "was in crisis" flag).
fn fire_recovery_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.recovery_beat_family.is_empty() {
        return false;
    }
    let recovered = sim.contract.as_ref().is_some_and(|c| {
        c.crisis_beats_fired > 0 && sim.population.unity >= cfg.recovery_beat_threshold
    });
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The crisis is past; re-arm the collapse beats against a future relapse.
        contract.crisis_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.recovery_beat_family, report);
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

/// Fire a depopulation beat (content-depth round 12): the crew's *headcount* — the
/// one major state dimension no beat watched. As the population falls to or below
/// each authored fraction of its founding size (high→low), a beat is forced — the
/// sealed ship's slow tragedy of a crew that only ever thins, marked at its stages.
/// Campaign-scoped (the counter persists across contracts, so a recruited-up ship
/// never re-marks a passed stage) but fires only during an active voyage. At most
/// one threshold per month.
fn fire_depopulation_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let fired = sim.depopulation_beats_fired as usize;
    if fired >= cfg.depopulation_beats.len() || sim.contract.is_none() {
        return false;
    }
    let founding = data.config.starting_population as f32;
    let threshold = (cfg.depopulation_beats[fired] * founding).ceil() as i64;
    if (sim.population.count as i64) > threshold {
        return false;
    }
    sim.depopulation_beats_fired += 1;
    let family = cfg.depopulation_beat_family.clone();
    force_family_beat(sim, data, &family, report);
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

/// Fire the homecoming beat (content-depth round 10): the first beat keyed to a
/// voyage *phase*. The moment the charter turns for home — enters its Return leg —
/// a single beat is forced from the homecoming family, the voyage's climactic
/// identity reckoning as a generation faces arrival at a homeport it no longer
/// resembles. Fires at most once per voyage; returns whether it replaced the roll.
fn fire_homecoming_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.homecoming_beat_family.is_empty() {
        return false;
    }
    let turning_home = sim.contract.as_ref().is_some_and(|c| {
        !c.homecoming_beat_fired && c.phase == crate::data::contracts::ContractPhase::Return
    });
    if !turning_home {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.homecoming_beat_fired = true;
    }
    force_family_beat(sim, data, &cfg.homecoming_beat_family, report);
    true
}

/// Fire a mid-voyage beat (content-depth campaign-skeleton round 21): the era
/// counterpart to the homecoming beat. The tick the voyage passes its temporal
/// midpoint *with home still ahead* (before the Return leg), a single beat is forced
/// from the mid-voyage family — the deep middle, when the founders are generations
/// dead and landfall generations away, and the crew live and die wholly in transit.
/// Fires at most once per voyage; a return-dominant charter whose midpoint already
/// falls in its Return leg leaves this to the homecoming beat instead.
fn fire_midvoyage_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.midvoyage_beat_family.is_empty() {
        return false;
    }
    let past_midpoint = sim.contract.as_ref().is_some_and(|c| {
        !c.midvoyage_beat_fired
            && c.phase != crate::data::contracts::ContractPhase::Return
            && c.months_elapsed * 2 >= c.total_months()
    });
    if !past_midpoint {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.midvoyage_beat_fired = true;
    }
    force_family_beat(sim, data, &cfg.midvoyage_beat_family, report);
    true
}

/// Fire a founding-era beat (content-depth campaign-skeleton round 22): the early
/// member of the era trio. The campaign-year the voyage passes `founding_beat_year` —
/// the founding generation, the ones who chose to leave, having by then largely passed,
/// and the ship handed for the first time wholly to those born to the void — a single
/// beat is forced from the founding family. Campaign-scoped: fires once ever (tracked on
/// `SimState`, not the contract), so a back-to-back second charter does not re-mark it.
/// Requires an active voyage, like the other beats.
fn fire_founding_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.founding_beat_family.is_empty()
        || cfg.founding_beat_year == 0
        || sim.founding_beat_fired
        || sim.contract.is_none()
        || sim.year() < cfg.founding_beat_year
    {
        return false;
    }
    sim.founding_beat_fired = true;
    let family = cfg.founding_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a power-transition beat (content-depth round 11): a beat keyed to a
/// *political* change rather than a stat or a time. When the dominant faction
/// differs from the one the skeleton last marked — demographic drift has grown a
/// minority into the majority, or a schism has unseated the largest people — a
/// beat is forced: the ship reckoning with new leadership. The first observation a
/// campaign only *records* the majority (no beat at launch). Fires on the change.
fn fire_power_transition_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let family = &data.config.campaign_skeleton.power_transition_beat_family;
    // Only a beat during an active voyage, and only on a *decisive* change of
    // majority — a clear plurality, not a launch-time tie-break wobble.
    if family.is_empty() || sim.contract.is_none() {
        return false;
    }
    let Some(current) = clear_majority_faction(sim) else {
        return false;
    };
    if sim.last_dominant_faction.is_empty() {
        // First clear majority this voyage: record it, do not fire.
        sim.last_dominant_faction = current;
        return false;
    }
    if current == sim.last_dominant_faction {
        return false;
    }
    sim.last_dominant_faction = current;
    let family = family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// The aboard faction that clearly runs the ship (content-depth round 11): the
/// largest, but only when it holds a decisive lead (over 1.1× the next, or sole
/// people aboard) — so a near-even split, where the majority wobbles on
/// tie-breaks, counts as *no* clear majority and marks no transition.
fn clear_majority_faction(sim: &SimState) -> Option<String> {
    let mut aboard: Vec<(&str, u32)> = sim
        .factions
        .iter()
        .filter(|f| f.is_aboard())
        .map(|f| (f.faction_id.as_str(), f.members))
        .collect();
    if aboard.is_empty() {
        return None;
    }
    aboard.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let (top_id, top) = aboard[0];
    let second = aboard.get(1).map_or(0, |x| x.1);
    (second == 0 || top as f32 > second as f32 * 1.1).then(|| top_id.to_owned())
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

/// Fire a succession beat (content-depth campaign-skeleton round 18 — the first
/// beat keyed to the new continuous-mortality system): the month a *sitting
/// leader dies in office* (`report.leader_died`, set by `mortality::monthly_tick`),
/// force a beat from the succession family so the ship reckons with an untried
/// command — a captain lost mid-voyage and an heir taking a chair they weren't
/// ready for — rather than the loss passing as a lone log line. A planned
/// retirement handoff does not fire it (only a death does). Fires only during a
/// voyage; consumes the flag so it fires once per death. Returns whether it
/// replaced this month's reactive roll.
fn fire_succession_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let family = data.config.campaign_skeleton.succession_beat_family.clone();
    if !report.leader_died || family.is_empty() || sim.contract.is_none() {
        return false;
    }
    report.leader_died = false;
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a long-reign beat (content-depth campaign-skeleton round 19 — the hopeful
/// mirror of the succession beat): once a sitting leader has held the first chair
/// for `long_reign_years`, force a beat from the long-reign family, the ship
/// reckoning with an era under one enduring hand — a thing grown rare now that
/// continuous mortality takes most leaders young. Fires once per reign (marked on
/// the dynasty, cleared by the next succession); voyage-only. Returns whether it
/// replaced the reactive roll.
fn fire_long_reign_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.long_reign_years == 0 || cfg.long_reign_beat_family.is_empty() || sim.contract.is_none()
    {
        return false;
    }
    let due = !sim.dynasty.long_reign_marked
        && sim.dynasty.leader().is_some()
        && sim.dynasty.leader_reign_years >= cfg.long_reign_years;
    if !due {
        return false;
    }
    sim.dynasty.long_reign_marked = true;
    let family = cfg.long_reign_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a dynasty-crisis beat (content-depth campaign-skeleton round 20 — the third
/// leadership beat, and the first keyed to the *dynasty's* own headcount): when the
/// founding line dwindles to or below `dynasty_crisis_size` — continuous mortality
/// outrunning the yearly renewal — force a beat from the crisis family, the ship
/// reckoning with the near-end of the family that has led it since the founding.
/// Fires once per brush with extinction; re-arms only once the line is restored to
/// its target size. Voyage-only. Returns whether it replaced the reactive roll.
fn fire_dynasty_crisis_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.dynasty_crisis_size == 0
        || cfg.dynasty_crisis_beat_family.is_empty()
        || sim.contract.is_none()
    {
        return false;
    }
    let count = sim.dynasty.members.len() as u32;
    // The line fully restored re-arms the beat against a future brush.
    if count >= data.config.mortality.dynasty_target_size {
        sim.dynasty.dynasty_crisis_marked = false;
        return false;
    }
    if sim.dynasty.dynasty_crisis_marked || count > cfg.dynasty_crisis_size {
        return false;
    }
    sim.dynasty.dynasty_crisis_marked = true;
    let family = cfg.dynasty_crisis_beat_family.clone();
    force_family_beat(sim, data, &family, report);
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
