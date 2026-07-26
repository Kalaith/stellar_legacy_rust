//! Terminal-styled UI shell: palette, shared widgets, action enum, and the
//! per-frame dispatch into screen modules.
//!
//! UI is a pure view layer: every function reads state and returns
//! `UiAction` intents; nothing here mutates the sim (CODE_STANDARDS §7).

pub mod chronicle;
pub mod contract_systems;
pub mod crew_dynasty;
pub mod dashboard;
pub mod event_modal;
pub mod game_over;
pub mod help;
pub mod main_menu;
pub mod market;
pub mod prep;
pub mod settings;
pub mod shell;
pub mod ship_builder;
pub mod ship_schematic;
pub mod subsystems;
pub mod welcome;
pub mod widgets;

pub use main_menu::*;
pub use shell::*;
pub use widgets::*;

use crate::chronicle::ChronicleStore;
use crate::data::events::EventCategory;
use crate::data::ship_components::ComponentKind;
use crate::data::GameData;
use crate::state::sim::{GameSpeed, SimState, TradeResource};
use crate::state::{MenuState, Screen};
use macroquad::prelude::*;
use macroquad_toolkit::achievements::Achievements;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt, VirtualUi};

pub const LOGICAL_WIDTH: f32 = 1280.0;
pub const LOGICAL_HEIGHT: f32 = 720.0;

/// Phosphor-terminal palette (GDD §0). The tube is monochrome: every color is
/// a brightness of one hue, selectable at runtime between amber (P3) and green
/// (P1) via [`term::set_phosphor`]. Alerts stay warm-red on both so danger
/// reads even on a green tube.
pub mod term {
    use crate::settings::Phosphor;
    use macroquad::prelude::Color;
    use std::cell::Cell;

    thread_local! {
        static PHOSPHOR: Cell<Phosphor> = const { Cell::new(Phosphor::Amber) };
    }

    /// Switch the active phosphor tube for all subsequent draws.
    pub fn set_phosphor(phosphor: Phosphor) {
        PHOSPHOR.with(|cell| cell.set(phosphor));
    }

    fn tube(amber: Color, green: Color) -> Color {
        match PHOSPHOR.with(Cell::get) {
            Phosphor::Amber => amber,
            Phosphor::Green => green,
        }
    }

    pub fn bg() -> Color {
        tube(
            Color::new(0.015, 0.012, 0.004, 1.0),
            Color::new(0.003, 0.016, 0.006, 1.0),
        )
    }

    pub fn panel() -> Color {
        tube(
            Color::new(0.06, 0.047, 0.012, 0.98),
            Color::new(0.015, 0.052, 0.023, 0.98),
        )
    }

    pub fn panel_header() -> Color {
        tube(
            Color::new(0.13, 0.095, 0.02, 1.0),
            Color::new(0.03, 0.12, 0.045, 1.0),
        )
    }

    pub fn primary() -> Color {
        tube(
            Color::new(1.0, 0.75, 0.14, 1.0),
            Color::new(0.42, 1.0, 0.56, 1.0),
        )
    }

    pub fn dim() -> Color {
        tube(
            Color::new(0.74, 0.54, 0.11, 1.0),
            Color::new(0.26, 0.74, 0.38, 1.0),
        )
    }

    pub fn faint() -> Color {
        tube(
            Color::new(0.44, 0.32, 0.08, 1.0),
            Color::new(0.14, 0.42, 0.2, 1.0),
        )
    }

    /// Success / value accent — a brighter tint of the tube hue.
    pub fn accent() -> Color {
        tube(
            Color::new(0.2, 1.0, 0.5, 1.0),
            Color::new(0.62, 1.0, 0.72, 1.0),
        )
    }

    /// Alert red — warm on both tubes so danger still reads on a green screen.
    pub fn alert() -> Color {
        Color::new(1.0, 0.32, 0.24, 1.0)
    }

    pub fn border() -> Color {
        tube(
            Color::new(0.82, 0.6, 0.14, 0.95),
            Color::new(0.3, 0.8, 0.42, 0.95),
        )
    }

    // Dark interactive surface fills (buttons, tabs, selectable rows), tinted to
    // the tube so nothing reads warm on the green screen.
    pub fn surface() -> Color {
        tube(
            Color::new(0.12, 0.092, 0.017, 1.0),
            Color::new(0.022, 0.075, 0.034, 1.0),
        )
    }

    pub fn surface_hover() -> Color {
        tube(
            Color::new(0.26, 0.19, 0.025, 1.0),
            Color::new(0.05, 0.16, 0.075, 1.0),
        )
    }

    pub fn surface_active() -> Color {
        tube(
            Color::new(0.3, 0.22, 0.03, 1.0),
            Color::new(0.06, 0.18, 0.085, 1.0),
        )
    }

    pub fn surface_disabled() -> Color {
        tube(
            Color::new(0.05, 0.04, 0.02, 1.0),
            Color::new(0.01, 0.035, 0.016, 1.0),
        )
    }

    pub fn surface_inset() -> Color {
        tube(
            Color::new(0.07, 0.055, 0.012, 1.0),
            Color::new(0.014, 0.05, 0.024, 1.0),
        )
    }
}

/// Every interaction the UI can request. Game logic applies these in
/// `game.rs`; adding an interaction means adding a variant here, never
/// mutating state from a panel.
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    // Menu
    SelectLegacy(usize),
    /// Toggle a founding faction in the new-game picker (W7).
    ToggleFaction(String),
    StartNewGame,
    ContinueGame,
    DeleteSave,
    /// Step from the main menu into the new-game (legacy/faction) picker.
    GoToNewGame,
    /// Step back from the new-game picker to the main menu.
    BackToMainMenu,
    /// Open the display/settings overlay from the main menu.
    OpenSettings,
    /// Quit the application from the main menu.
    ExitGame,
    // Global
    SaveGame,
    ToMenu,
    RetireVoyage,
    SelectScreen(Screen),
    // Gameplay verbs (GDD §4)
    /// Set the real-time auto-advance rate / pause (real-time loop §1).
    SetSpeed(GameSpeed),
    /// Turn the current mission for home early (W2). Only emitted underway.
    AbortMission,
    ResolveEvent(usize),
    ResolveDilemma(usize),
    RecruitCrew(String),
    TrainCrew(String),
    SelectHeir(u32),
    /// Put a charter under consideration in port — never starts it (W4).
    SelectCharter(String),
    /// Commit the selected charter and begin the voyage (W4) — the sole path
    /// that starts a contract.
    Launch,
    /// Refuel to a full tank in drydock (W4).
    Refuel,
    /// Stock spare parts in drydock (W4 provisioning, PREP screen).
    BuyParts(i64),
    /// Hide the first-voyage checklist for the rest of the campaign.
    DismissTutorial,
    PurchaseComponent(ComponentKind, String),
    FieldRepair(crate::simulation::ship::RepairKind),
    FullRepair,
    InstallSalvage(String),
    CommissionShip(String),
    /// Recruit a fresh people in drydock when short of the founding count (W7).
    RecruitFactionGroup(String),
    /// Subsystem verbs (W5): mend, upgrade (port), or train its knowledge.
    RepairSubsystem(String),
    UpgradeSubsystem(String),
    /// Fit a mission-reward subsystem version the ship has unlocked (2c) — free,
    /// drydock-only, distinct from the bought `UpgradeSubsystem`.
    InstallFitting(String),
    TrainSubsystemKnowledge(String),
    Buy(TradeResource, i64),
    Sell(TradeResource, i64),
    ToggleDelegation(EventCategory),
}
