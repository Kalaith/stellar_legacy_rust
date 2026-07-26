//! Beats: the authored moments a voyage surfaces when the ship's state, its
//! calendar or its charter says something is worth telling. `fire_due_beat`
//! is the one entry point; the families it dispatches to live alongside.

use crate::data::GameData;
use crate::simulation::event_resolver;
use crate::state::sim::SimState;

use super::{apply_pending_event, TickReport};

mod collapse;
mod recovery;
mod scripted;
mod succession;
mod threshold;

pub(crate) use collapse::*;
pub(crate) use recovery::*;
pub(crate) use scripted::*;
pub(crate) use succession::*;
pub(crate) use threshold::*;

/// Fire a due campaign beat (W6): if an unfired beat has come due this month,
/// mark it and force an event from its family (falling through to a normal roll
/// when the family is over-gated). Returns whether a beat replaced this month's
/// random roll.
pub(crate) fn fire_due_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
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

/// The aboard faction that clearly runs the ship (content-depth round 11): the
/// largest, but only when it holds a decisive lead (over 1.1× the next, or sole
/// people aboard) — so a near-even split, where the majority wobbles on
/// tie-breaks, counts as *no* clear majority and marks no transition.
pub(crate) fn clear_majority_faction(sim: &SimState) -> Option<String> {
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

/// Force one event from `family` (falling through to a normal reactive roll when
/// the family is over-gated), applying it. Shared by the threshold-beat firers.
pub(crate) fn force_family_beat(
    sim: &mut SimState,
    data: &GameData,
    family: &str,
    report: &mut TickReport,
) {
    let pending = event_resolver::roll_event_in_family(sim, data, family)
        .or_else(|| event_resolver::roll_event(sim, data));
    if let Some(pending) = pending {
        apply_pending_event(sim, data, pending, report);
    }
}

/// Force a specific event by id (content-depth): build a pending event for it and
/// apply it, bypassing gates — the shared path for both the scheduled follow-up
/// (round 9) and a charter's scripted timed beats (charters round 9).
pub(crate) fn force_event_beat(
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
