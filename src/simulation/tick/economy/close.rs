//! Closing the year: the ambient line, the fuel report, the market drift.

use crate::data::GameData;
use crate::simulation::market;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::factors::quiet_ambient_pool;

/// The last three facts of an economic year, in the order they are told.
pub(super) fn close_the_year(sim: &mut SimState, data: &GameData, _report: &mut TickReport) {
    let config = &data.config;

    // Content-depth voice (round 2): during a long event-less stretch, surface an
    // atmospheric "life aboard" line so the passing centuries read as lived-in.
    // Deterministic — fires once per `ambient_gap_years` of quiet, indexed by
    // year, no RNG, and never resets the event ramp.
    let fl = &config.flavor;
    if fl.ambient_gap_years > 0 {
        let years_since = sim.month_clock.saturating_sub(sim.last_event_month_clock) / 12;
        if years_since > 0 && years_since.is_multiple_of(fl.ambient_gap_years) {
            // The quiet reads differently as the ship changes (see
            // `quiet_ambient_pool`): the grim/flush *conditions* first, and failing
            // all of them, the plain "ordinary" quiet colored by *who runs the ship*.
            let pool = quiet_ambient_pool(sim, data);
            if !pool.is_empty() {
                let idx = (sim.year() / fl.ambient_gap_years) as usize % pool.len();
                sim.push_log(pool[idx].clone());
            }
        }
    }

    // Legible fuel provisioning (real-time loop follow-up: stat changes should
    // read as *something the ship did*). The tank sags monthly with the burn and
    // is topped up yearly by the drive's scoop — a sawtooth that used to move with
    // no word in the log. Periodically report the fuel actually gathered since the
    // last note, so the rise has an in-world cause. Self-throttling: the accrual
    // only grows while the tank has room to take on fuel (i.e. while a crossing is
    // drawing it down), so a ship sitting on a full tank on-station stays silent.
    if fl.fuel_report_gap_years > 0
        && !fl.fuel_gain.is_empty()
        && sim.year() > 0
        && sim.year().is_multiple_of(fl.fuel_report_gap_years)
    {
        let amount = (sim.fuel_scooped_accum * 100.0).round() as i64;
        if amount >= 5 {
            let idx = (sim.year() / fl.fuel_report_gap_years) as usize % fl.fuel_gain.len();
            sim.push_log(fl.fuel_gain[idx].replace("{amount}", &amount.to_string()));
            sim.fuel_scooped_accum = 0.0;
        }
    }

    // Market drift closes the economic year. Contract progress is monthly (W2)
    // and the event roll is monthly (W3) — both live in `advance` now; log
    // trimming happens once there too.
    market::drift_prices(sim);
}
