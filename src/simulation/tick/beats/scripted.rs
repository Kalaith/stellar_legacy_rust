//! Beats with an author behind them, plus the backstop against dead air.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::{clear_majority_faction, force_event_beat, force_family_beat};

/// Fire a reputation beat (content-depth round 16): the skeleton's first trigger on
/// the ship's *cumulative character* (it105), not a population stat. When the named
/// reputation trait crosses *into* a strong band — famously high or notoriously low —
/// force a beat, the ship reckoning with the name it has earned; a return to the
/// middle silently re-arms it. Fires only during a voyage; at most one per month.
pub(crate) fn fire_reputation_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a founding-era beat (content-depth campaign-skeleton round 22): the early
/// member of the era trio. The campaign-year the voyage passes `founding_beat_year` —
/// the founding generation, the ones who chose to leave, having by then largely passed,
/// and the ship handed for the first time wholly to those born to the void — a single
/// beat is forced from the founding family. Campaign-scoped: fires once ever (tracked on
/// `SimState`, not the contract), so a back-to-back second charter does not re-mark it.
/// Requires an active voyage, like the other beats.
pub(crate) fn fire_founding_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire the homecoming beat (content-depth round 10): the first beat keyed to a
/// voyage *phase*. The moment the charter turns for home — enters its Return leg —
/// a single beat is forced from the homecoming family, the voyage's climactic
/// identity reckoning as a generation faces arrival at a homeport it no longer
/// resembles. Fires at most once per voyage; returns whether it replaced the roll.
pub(crate) fn fire_homecoming_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a power-transition beat (content-depth round 11): a beat keyed to a
/// *political* change rather than a stat or a time. When the dominant faction
/// differs from the one the skeleton last marked — demographic drift has grown a
/// minority into the majority, or a schism has unseated the largest people — a
/// beat is forced: the ship reckoning with new leadership. The first observation a
/// campaign only *records* the majority (no beat at launch). Fires on the change.
pub(crate) fn fire_power_transition_beat(
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

/// Fire a charter's scripted timed beat (content-depth charters round 9): a
/// mission built around a reckoning on a known clock forces its next beat once
/// this voyage has run to its `at_year`. Beats are authored ascending and fire in
/// order, one per month; returns whether it replaced the reactive roll.
pub(crate) fn fire_charter_scheduled_beat(
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
pub(crate) fn fire_scheduled_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a dead-air backstop beat (content-depth round 5): once more than
/// `dead_air_years` have passed with no event, guarantee one rather than let the
/// voyage drift on empty — long eventless stretches are a coverage bug, not a
/// mercy. The family is drawn from `dead_air_pool` via the state RNG (so a seed
/// still replays), and forcing a beat resets the event clock. Only while a
/// contract is under way; off when `dead_air_years` is 0. Returns whether it
/// replaced this month's reactive roll.
pub(crate) fn fire_dead_air_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
