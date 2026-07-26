//! The ship's own voice speaks once on a turn, not every year after it.

use super::*;

#[test]
fn the_ship_remarks_when_its_hull_groans_or_rides_sound() {
    // Content-depth voice round 22: the hull voice, the first for the ship's own
    // body. A new-built hull is the silent baseline; crossing into a groaning band
    // surfaces one pooled line; a refit back to a sound band gets its own, opposite
    // line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(47);
    let fl = &data.config.flavor;
    assert!(
        fl.hull_voice_high > 0.0 && fl.hull_groaning.len() >= 3,
        "this test needs the hull voice enabled"
    );
    let low = fl.hull_voice_low;
    let high = fl.hull_voice_high;
    let hull_lines = |sim: &SimState| {
        let groan = &data.config.flavor.hull_groaning;
        let sound = &data.config.flavor.hull_sound;
        sim.log
            .iter()
            .filter(|l| groan.contains(&l.text) || sound.contains(&l.text))
            .count()
    };

    // A new-built hull is sound — the launch band is recorded, silent.
    sim.announce_hull_condition(&data);
    assert_eq!(hull_lines(&sim), 0, "a new-built hull is silent");

    // The hull wears past the low line: one line.
    sim.ship.hull_integrity = low - 0.05;
    sim.announce_hull_condition(&data);
    assert_eq!(hull_lines(&sim), 1, "an aging hull groans once");
    assert_eq!(sim.hull_voice_band, -1);

    // Still groaning — no reprint.
    sim.announce_hull_condition(&data);
    assert_eq!(hull_lines(&sim), 1, "staying worn is not re-announced");

    // A refit brings it back sound: a second, distinct line.
    sim.ship.hull_integrity = high + 0.05;
    sim.announce_hull_condition(&data);
    assert_eq!(hull_lines(&sim), 2, "a refit hull rides sound afresh");
    assert_eq!(sim.hull_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_its_air_goes_stale_or_clears() {
    // Content-depth voice round 23: the air (life-support) voice, the atmosphere twin
    // of the hull voice. A new ship's clean air is the silent baseline; crossing into
    // a stale band surfaces one pooled line; an overhaul back to fresh gets its own,
    // opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(51);
    let fl = &data.config.flavor;
    assert!(
        fl.air_voice_high > 0.0 && fl.air_stale.len() >= 3,
        "this test needs the air voice enabled"
    );
    let low = fl.air_voice_low;
    let high = fl.air_voice_high;
    let air_lines = |sim: &SimState| {
        let stale = &data.config.flavor.air_stale;
        let fresh = &data.config.flavor.air_fresh;
        sim.log
            .iter()
            .filter(|l| stale.contains(&l.text) || fresh.contains(&l.text))
            .count()
    };

    // A new ship breathes clean — the launch band is recorded, silent.
    sim.announce_air_condition(&data);
    assert_eq!(air_lines(&sim), 0, "a new ship's air is silent");

    // The air goes stale past the low line: one line.
    sim.ship.life_support = low - 0.05;
    sim.announce_air_condition(&data);
    assert_eq!(air_lines(&sim), 1, "staling air says so once");
    assert_eq!(sim.air_voice_band, -1);

    // Still stale — no reprint.
    sim.announce_air_condition(&data);
    assert_eq!(air_lines(&sim), 1, "staying stale is not re-announced");

    // An overhaul clears the air: a second, distinct line.
    sim.ship.life_support = high + 0.05;
    sim.announce_air_condition(&data);
    assert_eq!(air_lines(&sim), 2, "cleared air says so afresh");
    assert_eq!(sim.air_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_its_drive_runs_thin_or_full() {
    // Content-depth voice round 27: the drive (fuel) voice, the third ship-body voice, the
    // motion twin of the hull and air voices. A new ship's full tanks are the silent
    // baseline; the tanks running thin surfaces one pooled line; a resupply back to full
    // gets its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(59);
    let fl = &data.config.flavor;
    assert!(
        fl.fuel_voice_high > 0.0 && fl.drive_thin.len() >= 3,
        "this test needs the drive voice enabled"
    );
    let low = fl.fuel_voice_low;
    let high = fl.fuel_voice_high;
    let drive_lines = |sim: &SimState| {
        let thin = &data.config.flavor.drive_thin;
        let strong = &data.config.flavor.drive_strong;
        sim.log
            .iter()
            .filter(|l| thin.contains(&l.text) || strong.contains(&l.text))
            .count()
    };

    // A new ship sets out with full tanks — the launch band is recorded, silent.
    sim.announce_drive_condition(&data);
    assert_eq!(drive_lines(&sim), 0, "a new ship's full drive is silent");

    // The tanks run thin past the low line: one line.
    sim.ship.fuel = low - 0.05;
    sim.announce_drive_condition(&data);
    assert_eq!(drive_lines(&sim), 1, "a thinning drive says so once");
    assert_eq!(sim.fuel_voice_band, -1);

    // Still thin — no reprint.
    sim.announce_drive_condition(&data);
    assert_eq!(drive_lines(&sim), 1, "staying thin is not re-announced");

    // A resupply fills the tanks: a second, distinct line.
    sim.ship.fuel = high + 0.05;
    sim.announce_drive_condition(&data);
    assert_eq!(drive_lines(&sim), 2, "a refilled drive says so afresh");
    assert_eq!(sim.fuel_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_its_crew_swells_or_thins() {
    // Content-depth voice round 30: the headcount voice. A ship at its founding complement is
    // the silent baseline; the crew thinning below the line says so once; swelling above it
    // gets its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(60);
    let fl = &data.config.flavor;
    assert!(
        fl.crew_size_voice_high_ratio > 0.0 && fl.crew_swelling.len() >= 3,
        "this test needs the crew-size voice enabled"
    );
    let starting = data.config.starting_population as f32;
    let high = fl.crew_size_voice_high_ratio;
    let low = fl.crew_size_voice_low_ratio;
    let crew_lines = |sim: &SimState| {
        let swell = &data.config.flavor.crew_swelling;
        let thin = &data.config.flavor.crew_thinning;
        sim.log
            .iter()
            .filter(|l| swell.contains(&l.text) || thin.contains(&l.text))
            .count()
    };

    // A ship at its founding complement — the launch band is recorded, silent.
    sim.announce_crew_size_mood(&data);
    assert_eq!(
        crew_lines(&sim),
        0,
        "a ship at its founding complement is silent"
    );

    // The crew thins below the low line: one line.
    sim.population.count = ((low - 0.05) * starting) as u32;
    sim.announce_crew_size_mood(&data);
    assert_eq!(crew_lines(&sim), 1, "a thinning crew says so once");
    assert_eq!(sim.crew_size_voice_band, -1);

    // Still thin — no reprint.
    sim.announce_crew_size_mood(&data);
    assert_eq!(crew_lines(&sim), 1, "staying thin is not re-announced");

    // The crew swells above the high line: a second, distinct line.
    sim.population.count = ((high + 0.05) * starting) as u32;
    sim.announce_crew_size_mood(&data);
    assert_eq!(crew_lines(&sim), 2, "a swelling crew says so afresh");
    assert_eq!(sim.crew_size_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_its_coffers_run_flush_or_bare() {
    // Content-depth voice round 32: the treasury voice. A ship at its founding stake is the
    // silent baseline; the coffers running bare below the low line say so once; flush above the
    // high line gets its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(62);
    let fl = &data.config.flavor;
    assert!(
        fl.treasury_voice_high_ratio > 0.0 && fl.treasury_flush.len() >= 2,
        "this test needs the treasury voice enabled"
    );
    let starting = data.config.starting_resources.credits as f32;
    let high = fl.treasury_voice_high_ratio;
    let low = fl.treasury_voice_low_ratio;
    let treasury_lines = |sim: &SimState| {
        let flush = &data.config.flavor.treasury_flush;
        let bare = &data.config.flavor.treasury_bare;
        sim.log
            .iter()
            .filter(|l| flush.contains(&l.text) || bare.contains(&l.text))
            .count()
    };

    // A ship at its founding stake — the launch band is recorded, silent.
    sim.resources.credits = starting as i64;
    sim.announce_treasury_mood(&data);
    assert_eq!(
        treasury_lines(&sim),
        0,
        "a ship at its founding stake is silent"
    );

    // The coffers run bare below the low line: one line.
    sim.resources.credits = ((low - 0.05) * starting) as i64;
    sim.announce_treasury_mood(&data);
    assert_eq!(treasury_lines(&sim), 1, "a bare treasury says so once");
    assert_eq!(sim.treasury_voice_band, -1);

    // Still bare — no reprint.
    sim.announce_treasury_mood(&data);
    assert_eq!(treasury_lines(&sim), 1, "staying bare is not re-announced");

    // A run of pay fills the coffers past the high line: a second, distinct line.
    sim.resources.credits = ((high + 0.2) * starting) as i64;
    sim.announce_treasury_mood(&data);
    assert_eq!(treasury_lines(&sim), 2, "a flush treasury says so afresh");
    assert_eq!(sim.treasury_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_its_reactors_run_flush_or_dark() {
    // Content-depth voice round 33: the power voice, the treasury's energy sibling. A ship at its
    // founding stock (bracketed between the lines) is the silent baseline; the grid running dark
    // below the low line says so once; flush above the high line gets its own, opposite line;
    // staying put does not reprint.
    let (data, mut sim, _picks) = armed(64);
    let fl = &data.config.flavor;
    assert!(
        fl.power_voice_high > 0 && fl.power_flush.len() >= 2,
        "this test needs the power voice enabled"
    );
    let high = fl.power_voice_high;
    let low = fl.power_voice_low;
    let power_lines = |sim: &SimState| {
        let flush = &data.config.flavor.power_flush;
        let starved = &data.config.flavor.power_starved;
        sim.log
            .iter()
            .filter(|l| flush.contains(&l.text) || starved.contains(&l.text))
            .count()
    };

    // A ship at its founding stock — the launch band is recorded, silent.
    sim.resources.energy = data.config.starting_resources.energy;
    sim.announce_power_mood(&data);
    assert_eq!(
        power_lines(&sim),
        0,
        "a ship at its founding stock is silent"
    );

    // The grid runs dark at or below the low line: one line.
    sim.resources.energy = low - 100;
    sim.announce_power_mood(&data);
    assert_eq!(power_lines(&sim), 1, "a dark grid says so once");
    assert_eq!(sim.power_voice_band, -1);

    // Still dark — no reprint.
    sim.announce_power_mood(&data);
    assert_eq!(power_lines(&sim), 1, "staying dark is not re-announced");

    // The reactors run flush past the high line: a second, distinct line.
    sim.resources.energy = high + 500;
    sim.announce_power_mood(&data);
    assert_eq!(power_lines(&sim), 2, "flush reactors say so afresh");
    assert_eq!(sim.power_voice_band, 1);
}
