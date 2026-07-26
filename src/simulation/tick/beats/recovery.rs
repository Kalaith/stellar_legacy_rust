//! The mirror of collapse: a ship pulled back from the brink says so too.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::force_family_beat;

/// Fire a hull-recovery beat (content-depth campaign-skeleton round 32): the structural twin of
/// the crew-stat recovery beats (unity it13, stability it28, morale it30, loyalty it31), and the
/// *ascending* mirror of the it23 hull-collapse beat. Once the ship's frame has failed (a hull
/// beat fired, `hull_beats_fired > 0`) and `hull_integrity` then climbs back to or above the
/// recovery threshold — a genuine refit, set above the collapse red line for hysteresis — force a
/// beat (the crew confronting a vessel dragged back from structural failure and made whole again)
/// and reset the collapse counter so a later failure reckons anew. It gives the ship's body the
/// same fall-and-rise the crew's four erodable stats have, so a hull crisis is no longer a door
/// that only ever opens one way. Fires once per failure episode; at most one per month.
pub(crate) fn fire_hull_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.hull_recovery_beat_family.is_empty() || cfg.hull_recovery_beat_threshold <= 0.0 {
        return false;
    }
    let recovered = sim.contract.as_ref().is_some_and(|c| {
        c.hull_beats_fired > 0 && sim.ship.hull_integrity >= cfg.hull_recovery_beat_threshold
    });
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The frame is rebuilt; re-arm the collapse beat against a future failure.
        contract.hull_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.hull_recovery_beat_family, report);
    true
}

/// Fire an air-recovery beat (content-depth campaign-skeleton round 33): the atmosphere twin of the
/// it32 hull-recovery beat, and the *ascending* mirror of the it24 air-collapse beat. Once the
/// ship's air has failed (an air beat fired, `air_beats_fired > 0`) and `life_support` then climbs
/// back to or above the recovery threshold — a real overhaul, set above the collapse red line for
/// hysteresis — force a beat (the crew confronting a ship whose air was dragged back from
/// suffocation and made breathable again) and reset the collapse counter so a later failure reckons
/// anew. With this the ship's *air* joins its frame (it32) and its crew's four stats in sounding
/// both its breaking and its mending. Fires once per failure episode; at most one per month.
pub(crate) fn fire_air_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.air_recovery_beat_family.is_empty() || cfg.air_recovery_beat_threshold <= 0.0 {
        return false;
    }
    let recovered = sim.contract.as_ref().is_some_and(|c| {
        c.air_beats_fired > 0 && sim.ship.life_support >= cfg.air_recovery_beat_threshold
    });
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The air is overhauled; re-arm the collapse beat against a future failure.
        contract.air_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.air_recovery_beat_family, report);
    true
}

/// Fire a becalmed-recovery beat (content-depth campaign-skeleton round 34): the mobility twin of
/// the it32 hull-recovery and it33 air-recovery beats, and the *ascending* mirror of the it25
/// becalmed collapse beat — the seventh and last collapse beat to gain its recovery, so *every*
/// collapse beat now sounds both its breaking and its mending. Once the ship has been stranded (a
/// becalmed beat fired, `becalmed_beats_fired > 0`) and it *burns again* (`fuel_stall_years` back to
/// 0, the drive lit after years dead in the water), force a beat (the crew reckoning with a voyage
/// underway once more) and reset the collapse counter so a later stranding reckons anew. Unlike the
/// hull/air recoveries this needs no hysteresis threshold — the stall counter resets to 0 in one
/// step when the ship burns, so "moving again" is an unambiguous crossing. Fires once per stranding.
pub(crate) fn fire_becalmed_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.becalmed_recovery_beat_family.is_empty() {
        return false;
    }
    let recovered = sim
        .contract
        .as_ref()
        .is_some_and(|c| c.becalmed_beats_fired > 0 && sim.fuel_stall_years == 0);
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The drive is lit again; re-arm the collapse beat against a future stranding.
        contract.becalmed_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.becalmed_recovery_beat_family, report);
    true
}

/// Fire a recovery beat (content-depth round 13): the crisis beat's hopeful mirror.
/// Once the ship has fractured (a crisis beat fired) and its `unity` climbs back to
/// or above the recovery threshold, force a beat — the mending, a ship pulling back
/// from the brink — and reset the crisis counter so a relapse re-arms the collapse
/// beats. Fires once per crisis episode (the reset clears the "was in crisis" flag).
pub(crate) fn fire_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
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

/// Fire a governance-recovery beat (content-depth campaign-skeleton round 28): the *stability*
/// twin of the it13 unity recovery beat, and the hopeful mirror of the it15 stability-collapse
/// beats. Once the ship's institutions have failed (a stability beat fired) and its `stability`
/// climbs back to or above the recovery threshold, force a beat — the government rebuilt, the
/// councils reconvened, a ship pulling its own institutions back from anarchy — and reset the
/// stability-collapse counter so a relapse re-arms the collapse beats. Fires once per collapse
/// episode. Fires only during a voyage; at most one per month.
pub(crate) fn fire_stability_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.stability_recovery_beat_family.is_empty() || cfg.stability_recovery_beat_threshold <= 0.0
    {
        return false;
    }
    let recovered = sim.contract.as_ref().is_some_and(|c| {
        c.stability_beats_fired > 0
            && sim.population.stability >= cfg.stability_recovery_beat_threshold
    });
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The institutions are rebuilt; re-arm the collapse beats against a future relapse.
        contract.stability_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.stability_recovery_beat_family, report);
    true
}

/// Fire a heartening-recovery beat (content-depth campaign-skeleton round 30): the *morale* twin
/// of the it13 unity and it28 stability recovery beats, and the hopeful mirror of the it29 despair
/// beat. Once the crew has sunk into despair (a despair beat fired) and its `morale` then climbs
/// back to or above the recovery threshold, force a beat — a crew that had lost heart finding it
/// again, spirits lifting from the depths — and reset the despair counter so a relapse re-arms the
/// collapse beats. Distinct from the it8 flourish beat (which marks morale reaching a *golden age*
/// from any starting point): this marks the narrower, more moving thing of a crew climbing back to
/// a livable baseline *from the bottom*. Fires once per despair episode; at most one per month.
pub(crate) fn fire_heartening_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.heartening_recovery_beat_family.is_empty()
        || cfg.heartening_recovery_beat_threshold <= 0.0
    {
        return false;
    }
    let recovered = sim.contract.as_ref().is_some_and(|c| {
        c.despair_beats_fired > 0 && sim.population.morale >= cfg.heartening_recovery_beat_threshold
    });
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The crew has found its heart again; re-arm the despair beats against a relapse.
        contract.despair_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.heartening_recovery_beat_family, report);
    true
}

/// Fire a covenant-recovery beat (content-depth campaign-skeleton round 31): the *loyalty* twin
/// of the it13/it28/it30 recovery beats, and the last of the four decline stats to get one. Once
/// the founders' covenant has lapsed (a loyalty beat fired) and `legacy_loyalty` then climbs back
/// to or above the recovery threshold, force a beat — the covenant renewed, a generation that had
/// drifted from the founding cause returning to it, the charter re-embraced as binding — and reset
/// the loyalty-collapse counter so a relapse re-arms the collapse beats. Fires once per lapse
/// episode; at most one per month.
pub(crate) fn fire_loyalty_recovery_beat(
    sim: &mut SimState,
    data: &GameData,
    report: &mut TickReport,
) -> bool {
    let cfg = &data.config.campaign_skeleton;
    if cfg.loyalty_recovery_beat_family.is_empty() || cfg.loyalty_recovery_beat_threshold <= 0.0 {
        return false;
    }
    let recovered = sim.contract.as_ref().is_some_and(|c| {
        c.loyalty_beats_fired > 0
            && sim.population.legacy_loyalty >= cfg.loyalty_recovery_beat_threshold
    });
    if !recovered {
        return false;
    }
    if let Some(contract) = sim.contract.as_mut() {
        // The covenant is renewed; re-arm the collapse beats against a future lapse.
        contract.loyalty_beats_fired = 0;
    }
    force_family_beat(sim, data, &cfg.loyalty_recovery_beat_family, report);
    true
}
