//! Resolver tests, split by what the gate or outcome under test keys on.
//! The one shared fixture lives here.

use super::*;
use crate::data::events::EventCategory;
use crate::data::GameData;
use crate::state::sim::SimState;

mod aftermath;
mod chains;
mod charters;
mod complications;
mod gates;
mod outcome;
mod peoples;
mod provisioning;
mod reputation;
mod scheduled;

fn impact_cfg() -> RealTimeConfig {
    RealTimeConfig {
        seconds_per_month: 5.0,
        decision_timeout_secs: 30.0,
        impact_variance: 0.4,
        impact_min_magnitude_for_range: 20,
    }
}
