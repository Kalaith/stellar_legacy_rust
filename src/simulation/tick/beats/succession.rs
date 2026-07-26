//! The line renews: a leader dies, a long reign ends, a house dwindles.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::force_family_beat;

/// Fire a succession beat (content-depth campaign-skeleton round 18 — the first
/// beat keyed to the new continuous-mortality system): the month a *sitting
/// leader dies in office* (`report.leader_died`, set by `mortality::monthly_tick`),
/// force a beat from the succession family so the ship reckons with an untried
/// command — a captain lost mid-voyage and an heir taking a chair they weren't
/// ready for — rather than the loss passing as a lone log line. A planned
/// retirement handoff does not fire it (only a death does). Fires only during a
/// voyage; consumes the flag so it fires once per death. Returns whether it
/// replaced this month's reactive roll.
pub(crate) fn fire_succession_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
pub(crate) fn fire_long_reign_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
pub(crate) fn fire_dynasty_crisis_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
