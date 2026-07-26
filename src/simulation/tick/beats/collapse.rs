//! Descending beats: a system, an institution or the crew's heart fails.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::force_family_beat;

/// Fire a cohesion-collapse crisis beat (content-depth round 6): the *descending*
/// mirror of the drift/adaptation beats. As the people's `unity` falls to or
/// below each authored threshold (high→low), force a beat from the crisis family
/// — a fracturing ship generates its own reckoning rather than waiting on a
/// random roll. Fires at most one threshold per month; returns whether it
/// replaced the reactive roll.
pub(crate) fn fire_crisis_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a hull-collapse beat (content-depth campaign-skeleton round 23): the structural
/// twin of the subsystem-collapse beat — where that watches a *module's* condition, this
/// watches the *ship's own frame*. The month `hull_integrity` first falls to or below the
/// red line, a beat is forced (the crew confronting that the vessel itself is failing);
/// a refit back above the line re-arms it, so a ship rebuilt and let fail again reckons
/// anew. Fires only during a voyage; at most one per crossing.
pub(crate) fn fire_hull_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
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
    // Record the collapse so the hull-recovery beat (round 32) can reckon with a later refit;
    // fires only during a voyage, so a contract is present.
    if let Some(contract) = sim.contract.as_mut() {
        contract.hull_beats_fired += 1;
    }
    let family = cfg.hull_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire an air-collapse beat (content-depth campaign-skeleton round 24): the atmosphere
/// twin of the hull-collapse beat — where that watches the ship's frame, this watches its
/// air. The month `life_support` first falls to or below the red line, a beat is forced
/// (the crew confronting that the ship itself is suffocating); an overhaul back above the
/// line re-arms it. Fires only during a voyage; at most one per crossing.
pub(crate) fn fire_air_beat(sim: &mut SimState, data: &GameData, report: &mut TickReport) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.air_beat_family.is_empty() || cfg.air_beat_threshold <= 0.0 || sim.contract.is_none() {
        return false;
    }
    let band = if sim.ship.life_support <= cfg.air_beat_threshold {
        -1
    } else {
        0
    };
    if band == sim.air_beat_band {
        return false;
    }
    sim.air_beat_band = band;
    if band == 0 {
        // The air recovered above the red line — re-arm, but do not fire.
        return false;
    }
    // Record the collapse so the air-recovery beat (round 33) can reckon with a later overhaul;
    // fires only during a voyage, so a contract is present.
    if let Some(contract) = sim.contract.as_mut() {
        contract.air_beats_fired += 1;
    }
    let family = cfg.air_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a becalmed beat (content-depth campaign-skeleton round 25): the *mobility* twin
/// of the hull/air *integrity* collapse beats. Once the ship has been fuel-stalled — a
/// Travel leg dry, unable to burn — for `becalmed_beat_years` running, a beat is forced
/// (the crew confronting a ship that cannot make its heading); a year that burns again
/// re-arms it. Fires only during a voyage; at most one per stranding.
pub(crate) fn fire_becalmed_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.becalmed_beat_family.is_empty() || cfg.becalmed_beat_years == 0 || sim.contract.is_none()
    {
        return false;
    }
    let band = if sim.fuel_stall_years >= cfg.becalmed_beat_years {
        -1
    } else {
        0
    };
    if band == sim.becalmed_beat_band {
        return false;
    }
    sim.becalmed_beat_band = band;
    if band == 0 {
        // The ship burns again — re-arm, but do not fire.
        return false;
    }
    // Record the stranding so the becalmed-recovery beat (round 34) can reckon with the drive being
    // lit again; fires only during a voyage, so a contract is present.
    if let Some(contract) = sim.contract.as_mut() {
        contract.becalmed_beats_fired += 1;
    }
    let family = cfg.becalmed_beat_family.clone();
    force_family_beat(sim, data, &family, report);
    true
}

/// Fire a subsystem-collapse beat (content-depth round 17): the first forced beat
/// keyed to a *subsystem's condition* — the physical-crisis dimension the beat
/// lattice never watched. The first tick a configured module's condition falls to or
/// below its red line, a beat is forced from its family (a keystone that has truly
/// failed is a defining voyage crisis, guaranteed a reckoning rather than left to a
/// reactive roll). Campaign-scoped, once per module a voyage. Fires only during a
/// voyage; at most one per month.
pub(crate) fn fire_subsystem_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a stability-collapse beat (content-depth round 15): the last population stat
/// to get a beat. As `stability` falls to or below each authored threshold (high→
/// low), force a beat — not the people fracturing (crisis) nor the founders' authority
/// lapsing (loyalty), but the ship's own institutions ceasing to function. Fires at
/// most one threshold per month; returns whether it replaced the reactive roll.
pub(crate) fn fire_stability_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a loyalty-collapse beat (content-depth round 14): the last identity stat
/// to get a beat. As `legacy_loyalty` falls to or below each authored threshold
/// (high→low), force a beat — not the cultural drift the drift beats mark but the
/// political one, the founders' covenant lapsing. Fires at most one threshold per
/// month; returns whether it replaced the reactive roll.
pub(crate) fn fire_loyalty_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a morale-collapse *despair* beat (content-depth campaign-skeleton round 29): the
/// descending negative pole of the round-8 flourish beat. Where flourish forces a golden-age
/// reckoning as morale climbs past its high thresholds, this forces one as morale *crashes* past
/// its low ones — the crew sinking into a collective despair no other beat marked (the crisis
/// beat watches the ship *fracturing*, this watches it simply *losing heart*). As `morale` falls
/// to or below each authored threshold (high→low), a beat is forced. Fires at most one threshold
/// per month.
pub(crate) fn fire_despair_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    let crossed = sim.contract.as_ref().is_some_and(|c| {
        (c.despair_beats_fired as usize) < cfg.despair_beats.len()
            && sim.population.morale <= cfg.despair_beats[c.despair_beats_fired as usize]
    });
    if !crossed {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        contract.despair_beats_fired += 1;
    }
    force_family_beat(sim, data, &cfg.despair_beat_family, report);
    true
}
