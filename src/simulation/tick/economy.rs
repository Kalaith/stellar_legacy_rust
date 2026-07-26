//! The economic year (GDD §5.1), applied whole on each year boundary of the
//! month clock (W3): production, food upkeep, ship wear, voyage drift,
//! generation/succession, and market drift — the W1-tuned yearly math, split
//! out of `tick.rs` to keep the advance loop readable and the file under the
//! size limit.
//!
//! The year runs in six phases, each in its own module and each applied whole
//! in the order below. They are ordered, not independent: production settles
//! before the morale it feeds, and the modules wear before they are given a
//! voice.

mod close;
pub(super) mod factors;
mod generation;
mod morale;
mod produce;
mod voice;
mod wear;

use close::close_the_year;
use generation::turn_the_generation;
use morale::settle_morale_and_politics;
use produce::produce_and_feed;
use voice::decay_modules_and_speak;
use wear::wear_the_ship;

use crate::data::GameData;
use crate::state::sim::SimState;

use super::TickReport;

/// One full economic year (GDD §5.1), applied on a year boundary: production,
/// food upkeep, wear, drift, generation/succession, contract progress, market.
/// Exactly the W1-tuned yearly math — only the clock advance and the (now
/// monthly) event roll live outside it (W3).
pub(super) fn year_boundary_tick(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    produce_and_feed(sim, data, report);
    settle_morale_and_politics(sim, data, report);
    wear_the_ship(sim, data, report);
    decay_modules_and_speak(sim, data, report);
    turn_the_generation(sim, data, report);
    close_the_year(sim, data, report);
}
