//! Ascending beats: the ship crosses a line and the voyage remarks on it.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::force_family_beat;

/// Fire a cultural-drift threshold beat (content-depth round 2): the first month
/// the people's `cultural_drift` reaches the next authored threshold, force a beat
/// from the drift family so the Long-Term Expedition beats read as consequences
/// of how far the voyage has changed the crew. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
pub(crate) fn fire_drift_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
pub(crate) fn fire_adaptation_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire an adaptation-divergence beat (content-depth campaign-skeleton round 26): the
/// *crew-body* twin of the hull/air/becalmed *ship-body* crisis beats, and the terminal
/// counterpart to the gentle ascending `adaptation_beats` milestones. The month the people's
/// `adaptation` first rises to or above the red line — grown so shipborn they can no longer
/// survive a planet, the founding mission physically impossible — a beat is forced (the crew
/// confronting that they have become the ship's own kind); a fall back below (a strong
/// infirmary holding the baseline) re-arms it. Fires only during a voyage; at most one per
/// crossing.
pub(crate) fn fire_divergence_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.divergence_beat_family.is_empty()
        || cfg.divergence_beat_threshold <= 0.0
        || sim.contract.is_none()
    {
        return false;
    }
    let band = if sim.population.adaptation >= cfg.divergence_beat_threshold {
        1
    } else {
        0
    };
    if band == sim.adaptation_divergence_band {
        return false;
    }
    sim.adaptation_divergence_band = band;
    if band == 0 {
        // The crew fell back below the red line — re-arm, but do not fire.
        return false;
    }
    let family = cfg.divergence_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a cultural-divergence beat (content-depth campaign-skeleton round 27): the *cultural*
/// twin of the it26 adaptation-divergence beat (their *bodies*), and the terminal counterpart
/// to the gentle ascending `drift_beats` milestones. The month the crew's `cultural_drift`
/// first rises to or above the red line — drifted so far that the founders' charter is no
/// longer a living instruction but a dead language, the mission carried by rote by people who
/// no longer understand its why — a beat is forced; a fall back below (a strong archive
/// reviving the old ways) re-arms it. Fires only during a voyage; at most one per crossing.
pub(crate) fn fire_cultural_divergence_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.cultural_divergence_beat_family.is_empty()
        || cfg.cultural_divergence_beat_threshold <= 0.0
        || sim.contract.is_none()
    {
        return false;
    }
    let band = if sim.population.cultural_drift >= cfg.cultural_divergence_beat_threshold {
        1
    } else {
        0
    };
    if band == sim.cultural_divergence_band {
        return false;
    }
    sim.cultural_divergence_band = band;
    if band == 0 {
        // The culture drifted back below the red line — re-arm, but do not fire.
        return false;
    }
    let family = cfg.cultural_divergence_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a depopulation beat (content-depth round 12): the crew's *headcount* — the
/// one major state dimension no beat watched. As the population falls to or below
/// each authored fraction of its founding size (high→low), a beat is forced — the
/// sealed ship's slow tragedy of a crew that only ever thins, marked at its stages.
/// Campaign-scoped (the counter persists across contracts, so a recruited-up ship
/// never re-marks a passed stage) but fires only during an active voyage. At most
/// one threshold per month.
pub(crate) fn fire_depopulation_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a flourish beat (content-depth round 8): the *ascending* positive pole of
/// the crisis beat. The first month the people's `morale` climbs to or past each
/// authored threshold (low→high), force a beat from the flourish family — a
/// thriving, well-stewarded ship surfaces its own golden age instead of the
/// skeleton only ever answering to trouble. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
pub(crate) fn fire_flourish_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
pub(crate) fn fire_objective_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
pub(crate) fn fire_anniversary_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a mid-voyage beat (content-depth campaign-skeleton round 21): the era
/// counterpart to the homecoming beat. The tick the voyage passes its temporal
/// midpoint *with home still ahead* (before the Return leg), a single beat is forced
/// from the mid-voyage family — the deep middle, when the founders are generations
/// dead and landfall generations away, and the crew live and die wholly in transit.
/// Fires at most once per voyage; a return-dominant charter whose midpoint already
/// falls in its Return leg leaves this to the homecoming beat instead.
pub(crate) fn fire_midvoyage_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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
