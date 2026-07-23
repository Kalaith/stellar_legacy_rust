//! Active-campaign state: the simulation plus which screen tab is open.
//!
//! Everything that must survive a save/load lives in `SimState`; `Screen` is
//! session-local UI state and deliberately not serialized.

use crate::state::sim::SimState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    /// In-port charter board / PREP / homecoming (docked only).
    Drydock,
    ShipBuilder,
    Subsystems,
    CrewDynasty,
    /// Active-contract progress (under way only).
    Contract,
    Market,
    Chronicle,
}

impl Screen {
    /// Tabs shown while docked (in port): the refit-and-choose set, with the
    /// DRYDOCK board and MARKET, but no active CONTRACT (real-time loop §5).
    pub const DOCKED: [Screen; 7] = [
        Screen::Dashboard,
        Screen::Drydock,
        Screen::ShipBuilder,
        Screen::Subsystems,
        Screen::CrewDynasty,
        Screen::Market,
        Screen::Chronicle,
    ];

    /// Tabs shown under way (on a mission): the operations set, with the active
    /// CONTRACT but no DRYDOCK board and no MARKET (trading is a port activity).
    pub const UNDERWAY: [Screen; 6] = [
        Screen::Dashboard,
        Screen::ShipBuilder,
        Screen::Subsystems,
        Screen::CrewDynasty,
        Screen::Contract,
        Screen::Chronicle,
    ];

    /// The tab set for the current voyage state (real-time loop §5).
    pub fn tabs(in_port: bool) -> &'static [Screen] {
        if in_port {
            &Self::DOCKED
        } else {
            &Self::UNDERWAY
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Screen::Dashboard => "DASHBOARD",
            Screen::Drydock => "DRYDOCK",
            Screen::ShipBuilder => "SHIP",
            Screen::Subsystems => "SUBSYSTEMS",
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

#[cfg(test)]
mod tests {
    use super::Screen;

    #[test]
    fn tab_set_changes_with_voyage_state() {
        // Docked: the refit board (DRYDOCK + MARKET), no active CONTRACT.
        let docked = Screen::tabs(true);
        assert!(docked.contains(&Screen::Drydock));
        assert!(docked.contains(&Screen::Market));
        assert!(!docked.contains(&Screen::Contract));

        // Under way: the operations set (CONTRACT), no DRYDOCK board, no MARKET.
        let underway = Screen::tabs(false);
        assert!(underway.contains(&Screen::Contract));
        assert!(!underway.contains(&Screen::Drydock));
        assert!(!underway.contains(&Screen::Market));

        // The dashboard is always reachable in both states.
        assert!(docked.contains(&Screen::Dashboard));
        assert!(underway.contains(&Screen::Dashboard));
    }
}
