//! Active-campaign state: the simulation plus which screen tab is open.
//!
//! Everything that must survive a save/load lives in `SimState`; `Screen` is
//! session-local UI state and deliberately not serialized.

use crate::state::sim::SimState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    ShipBuilder,
    CrewDynasty,
    Contract,
    Market,
    Chronicle,
}

impl Screen {
    pub const ALL: [Screen; 6] = [
        Screen::Dashboard,
        Screen::ShipBuilder,
        Screen::CrewDynasty,
        Screen::Contract,
        Screen::Market,
        Screen::Chronicle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Screen::Dashboard => "DASHBOARD",
            Screen::ShipBuilder => "SHIP",
            Screen::CrewDynasty => "CREW & DYNASTY",
            Screen::Contract => "CONTRACT",
            Screen::Market => "MARKET",
            Screen::Chronicle => "CHRONICLE",
        }
    }
}

pub struct GameplayState {
    pub sim: SimState,
    pub screen: Screen,
}

impl GameplayState {
    pub fn new(sim: SimState) -> Self {
        Self {
            sim,
            screen: Screen::Dashboard,
        }
    }
}
