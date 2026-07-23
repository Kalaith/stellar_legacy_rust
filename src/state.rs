//! Top-level game state machine (GDD §11).
//!
//! Exactly one `GameState` is active. State updates request changes by
//! returning a `StateTransition`; `Game::transition` applies it explicitly.

pub mod gameplay;
pub mod menu;
pub mod sim;

pub use gameplay::{GameplayState, Screen};
pub use menu::{MenuPhase, MenuState};
pub use sim::SimState;

pub enum GameState {
    Menu(MenuState),
    Gameplay(Box<GameplayState>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateTransition {
    /// Start a fresh campaign with the chosen legacy, RNG seed, and the three
    /// founding factions the picker selected (W7).
    NewCampaign {
        legacy_id: String,
        seed: u64,
        faction_ids: Vec<String>,
    },
    /// Load the autosave slot and enter gameplay.
    LoadCampaign,
    /// Autosave (if in gameplay) and return to the main menu.
    ToMenu,
}
