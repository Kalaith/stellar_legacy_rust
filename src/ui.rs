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
pub mod market;
pub mod ship_builder;

use crate::chronicle::ChronicleStore;
use crate::data::events::EventCategory;
use crate::data::ship_components::ComponentKind;
use crate::data::GameData;
use crate::state::sim::{SimState, TradeResource};
use crate::state::{MenuState, Screen};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt, VirtualUi};

pub const LOGICAL_WIDTH: f32 = 1280.0;
pub const LOGICAL_HEIGHT: f32 = 720.0;

/// Phosphor-terminal palette carried over from the web original (GDD §0):
/// amber primary #FFB000, green success #00FF66, red alerts.
pub mod term {
    use macroquad::prelude::Color;

    pub const BG: Color = Color::new(0.02, 0.018, 0.005, 1.0);
    pub const PANEL: Color = Color::new(0.055, 0.045, 0.012, 0.97);
    pub const PANEL_HEADER: Color = Color::new(0.09, 0.07, 0.015, 1.0);
    pub const AMBER: Color = Color::new(1.0, 0.69, 0.0, 1.0);
    pub const AMBER_DIM: Color = Color::new(0.62, 0.44, 0.05, 1.0);
    pub const AMBER_FAINT: Color = Color::new(0.35, 0.26, 0.05, 1.0);
    pub const GREEN: Color = Color::new(0.0, 1.0, 0.4, 1.0);
    pub const RED: Color = Color::new(1.0, 0.28, 0.2, 1.0);
    pub const BORDER: Color = Color::new(0.62, 0.44, 0.05, 0.8);
}

/// Every interaction the UI can request. Game logic applies these in
/// `game.rs`; adding an interaction means adding a variant here, never
/// mutating state from a panel.
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    // Menu
    SelectLegacy(usize),
    StartNewGame,
    ContinueGame,
    DeleteSave,
    // Global
    SaveGame,
    ToMenu,
    SelectScreen(Screen),
    // Gameplay verbs (GDD §4)
    AdvanceYear,
    ResolveEvent(usize),
    ResolveDilemma(usize),
    RecruitCrew(String),
    TrainCrew(String),
    SelectHeir(u32),
    AcceptContract(String),
    PurchaseComponent(ComponentKind, String),
    Buy(TradeResource, i64),
    Sell(TradeResource, i64),
    ToggleDelegation(EventCategory),
}

// ---------------------------------------------------------------------------
// Shared widgets
// ---------------------------------------------------------------------------

pub fn term_panel(rect: Rect, title: Option<&str>) {
    let style = SurfaceStyle::new(term::PANEL)
        .with_border(1.0, term::BORDER)
        .with_header(34.0, term::PANEL_HEADER)
        .with_header_divider(1.0, term::BORDER);
    if let Some(title) = title {
        draw_surface_with_title(rect, Some(title), &style, TextStyle::new(17.0, term::AMBER));
    } else {
        draw_surface(
            rect,
            &SurfaceStyle::new(term::PANEL).with_border(1.0, term::BORDER),
        );
    }
}

pub fn term_button(rect: Rect, label: &str, enabled: bool, mouse: Vec2) -> bool {
    let hovered = enabled && rect.contains_point(mouse);
    let fill = if !enabled {
        Color::new(0.05, 0.04, 0.02, 1.0)
    } else if hovered {
        Color::new(0.22, 0.16, 0.02, 1.0)
    } else {
        Color::new(0.11, 0.085, 0.015, 1.0)
    };
    let border = if enabled {
        term::AMBER_DIM
    } else {
        term::AMBER_FAINT
    };
    draw_surface(rect, &SurfaceStyle::new(fill).with_border(1.0, border));
    draw_text_centered_in_box_ex(
        label,
        rect.x + 6.0,
        rect.y,
        rect.w - 12.0,
        rect.h,
        TextStyle::new(
            16.0,
            if enabled {
                term::AMBER
            } else {
                term::AMBER_FAINT
            },
        ),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}

pub fn term_meter(rect: Rect, value: f32, max: f32, label: &str) {
    let critical = max > 0.0 && value / max < 0.35;
    meter(
        rect,
        value,
        max,
        if critical { term::RED } else { term::GREEN },
        Some(label),
    );
}

pub fn stat_line(x: f32, y: f32, label: &str, value: &str, value_color: Color) {
    draw_ui_text_ex(label, x, y, TextStyle::new(15.0, term::AMBER_DIM).params());
    draw_text_right(value, x + 250.0, y, TextStyle::new(15.0, value_color));
}

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

pub struct MenuCtx<'a> {
    pub data: &'a GameData,
    pub menu: &'a MenuState,
    pub legacy_ids: &'a [String],
    pub ui: &'a VirtualUi,
}

pub fn draw_menu(ctx: MenuCtx<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    draw_ui_text_ex(
        "STELLAR LEGACY",
        LOGICAL_WIDTH / 2.0 - 190.0,
        130.0,
        TextStyle::new(48.0, term::AMBER).params(),
    );
    draw_ui_text_ex(
        "// generational starship command //",
        LOGICAL_WIDTH / 2.0 - 165.0,
        165.0,
        TextStyle::new(17.0, term::AMBER_DIM).params(),
    );

    let panel = Rect::new(LOGICAL_WIDTH / 2.0 - 320.0, 210.0, 640.0, 420.0);
    term_panel(panel, Some("FOUNDING CHARTER"));
    let content = panel.inset(24.0);
    let mut y = content.y + 40.0;

    draw_ui_text_ex(
        "Choose the legacy that will steer your bloodline:",
        content.x,
        y,
        TextStyle::new(16.0, term::AMBER_DIM).params(),
    );
    y += 18.0;

    for (i, legacy_id) in ctx.legacy_ids.iter().enumerate() {
        let Some(legacy) = ctx.data.legacies.get(legacy_id) else {
            continue;
        };
        let rect = Rect::new(content.x, y + 8.0, content.w, 62.0);
        let selected = i == ctx.menu.selected_legacy;
        let fill = if selected {
            Color::new(0.16, 0.12, 0.02, 1.0)
        } else {
            Color::new(0.07, 0.055, 0.012, 1.0)
        };
        draw_surface(
            rect,
            &SurfaceStyle::new(fill).with_border(
                1.0,
                if selected {
                    term::AMBER
                } else {
                    term::AMBER_FAINT
                },
            ),
        );
        draw_ui_text_ex(
            &legacy.name,
            rect.x + 14.0,
            rect.y + 24.0,
            TextStyle::new(18.0, if selected { term::GREEN } else { term::AMBER }).params(),
        );
        draw_text_block(
            &legacy.description,
            rect.x + 14.0,
            rect.y + 32.0,
            rect.w - 28.0,
            26.0,
            13.0,
            2.0,
            term::AMBER_DIM,
        );
        if rect.contains_point(mouse) && is_mouse_button_released(MouseButton::Left) {
            actions.push(UiAction::SelectLegacy(i));
        }
        y += 70.0;
    }

    y += 20.0;
    let btn_w = (content.w - 20.0) / 3.0;
    if term_button(
        Rect::new(content.x, y, btn_w, 44.0),
        "BEGIN VOYAGE",
        true,
        mouse,
    ) {
        actions.push(UiAction::StartNewGame);
    }
    if term_button(
        Rect::new(content.x + btn_w + 10.0, y, btn_w, 44.0),
        "CONTINUE",
        ctx.menu.save_exists,
        mouse,
    ) {
        actions.push(UiAction::ContinueGame);
    }
    if term_button(
        Rect::new(content.x + (btn_w + 10.0) * 2.0, y, btn_w, 44.0),
        "DELETE SAVE",
        ctx.menu.save_exists,
        mouse,
    ) {
        actions.push(UiAction::DeleteSave);
    }

    actions
}

// ---------------------------------------------------------------------------
// Gameplay shell: header, tabs, screen dispatch, event modal
// ---------------------------------------------------------------------------

pub struct GameplayCtx<'a> {
    pub data: &'a GameData,
    pub sim: &'a SimState,
    pub screen: Screen,
    pub chronicle: &'a ChronicleStore,
    pub ui: &'a VirtualUi,
}

pub fn draw_gameplay(ctx: GameplayCtx<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    draw_header(&ctx);
    draw_tabs(&ctx, mouse, &mut actions);

    let content = Rect::new(16.0, 128.0, LOGICAL_WIDTH - 32.0, LOGICAL_HEIGHT - 144.0);
    match ctx.screen {
        Screen::Dashboard => dashboard::draw(&ctx, content, mouse, &mut actions),
        Screen::ShipBuilder => ship_builder::draw(&ctx, content, mouse, &mut actions),
        Screen::CrewDynasty => crew_dynasty::draw(&ctx, content, mouse, &mut actions),
        Screen::Contract => contract_systems::draw(&ctx, content, mouse, &mut actions),
        Screen::Market => market::draw(&ctx, content, mouse, &mut actions),
        Screen::Chronicle => chronicle::draw(&ctx, content, mouse, &mut actions),
    }

    // A pending council decision blocks everything else (GDD §9 step 4):
    // discard screen intents and only accept the modal's.
    if ctx.sim.pending_event.is_some() {
        actions.clear();
        event_modal::draw(&ctx, mouse, &mut actions);
    } else if ctx.sim.pending_dilemma.is_some() {
        actions.clear();
        event_modal::draw_dilemma(&ctx, mouse, &mut actions);
    }

    actions
}

fn draw_header(ctx: &GameplayCtx<'_>) {
    let rect = Rect::new(16.0, 12.0, LOGICAL_WIDTH - 32.0, 58.0);
    term_panel(rect, None);

    let sim = ctx.sim;
    draw_ui_text_ex(
        &ctx.data.config.display_name.to_uppercase(),
        rect.x + 16.0,
        rect.y + 36.0,
        TextStyle::new(24.0, term::AMBER).params(),
    );

    let leader = sim
        .dynasty
        .leader()
        .map(|l| format!("{} ({})", l.name, l.age))
        .unwrap_or_else(|| "NO LEADER".to_owned());
    let legacy = ctx
        .data
        .legacies
        .get(&sim.legacy.legacy_id)
        .map(|l| l.name.clone())
        .unwrap_or_default();
    draw_ui_text_ex(
        &format!(
            "YEAR {}  |  GEN {}  |  {}  |  {}",
            sim.year, sim.dynasty.generation, legacy, leader
        ),
        rect.x + 330.0,
        rect.y + 36.0,
        TextStyle::new(16.0, term::AMBER_DIM).params(),
    );

    draw_text_right(
        &format!(
            "CR {}  EN {}  MIN {}  FOOD {}  INF {}",
            sim.resources.credits,
            sim.resources.energy,
            sim.resources.minerals,
            sim.resources.food,
            sim.resources.influence
        ),
        rect.right() - 16.0,
        rect.y + 36.0,
        TextStyle::new(15.0, term::GREEN),
    );
}

fn draw_tabs(ctx: &GameplayCtx<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let tabs = Screen::ALL;
    let total_w = LOGICAL_WIDTH - 32.0 - 220.0;
    let tab_w = (total_w - (tabs.len() as f32 - 1.0) * 6.0) / tabs.len() as f32;
    for (i, screen) in tabs.iter().enumerate() {
        let rect = Rect::new(16.0 + i as f32 * (tab_w + 6.0), 80.0, tab_w, 38.0);
        let active = *screen == ctx.screen;
        let fill = if active {
            Color::new(0.2, 0.15, 0.02, 1.0)
        } else {
            Color::new(0.07, 0.055, 0.012, 1.0)
        };
        draw_surface(
            rect,
            &SurfaceStyle::new(fill).with_border(
                1.0,
                if active {
                    term::AMBER
                } else {
                    term::AMBER_FAINT
                },
            ),
        );
        draw_text_centered_in_box_ex(
            screen.label(),
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            TextStyle::new(14.0, if active { term::GREEN } else { term::AMBER_DIM }),
        );
        if !active && rect.contains_point(mouse) && is_mouse_button_released(MouseButton::Left) {
            actions.push(UiAction::SelectScreen(*screen));
        }
    }

    if term_button(
        Rect::new(LOGICAL_WIDTH - 232.0, 80.0, 104.0, 38.0),
        "SAVE",
        true,
        mouse,
    ) {
        actions.push(UiAction::SaveGame);
    }
    if term_button(
        Rect::new(LOGICAL_WIDTH - 120.0, 80.0, 104.0, 38.0),
        "MENU",
        true,
        mouse,
    ) {
        actions.push(UiAction::ToMenu);
    }
}
