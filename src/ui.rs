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
pub mod market;
pub mod settings;
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
            Color::new(0.02, 0.018, 0.005, 1.0),
            Color::new(0.004, 0.02, 0.008, 1.0),
        )
    }
    pub fn panel() -> Color {
        tube(
            Color::new(0.055, 0.045, 0.012, 0.97),
            Color::new(0.014, 0.05, 0.022, 0.97),
        )
    }
    pub fn panel_header() -> Color {
        tube(
            Color::new(0.09, 0.07, 0.015, 1.0),
            Color::new(0.02, 0.08, 0.03, 1.0),
        )
    }
    pub fn primary() -> Color {
        tube(
            Color::new(1.0, 0.69, 0.0, 1.0),
            Color::new(0.3, 1.0, 0.45, 1.0),
        )
    }
    pub fn dim() -> Color {
        tube(
            Color::new(0.62, 0.44, 0.05, 1.0),
            Color::new(0.16, 0.6, 0.28, 1.0),
        )
    }
    pub fn faint() -> Color {
        tube(
            Color::new(0.35, 0.26, 0.05, 1.0),
            Color::new(0.09, 0.32, 0.15, 1.0),
        )
    }
    /// Success / value accent — a brighter tint of the tube hue.
    pub fn accent() -> Color {
        tube(
            Color::new(0.0, 1.0, 0.4, 1.0),
            Color::new(0.6, 1.0, 0.7, 1.0),
        )
    }
    /// Alert red — warm on both tubes so danger still reads on a green screen.
    pub fn alert() -> Color {
        Color::new(1.0, 0.28, 0.2, 1.0)
    }
    pub fn border() -> Color {
        tube(
            Color::new(0.62, 0.44, 0.05, 0.8),
            Color::new(0.16, 0.6, 0.28, 0.8),
        )
    }

    // Dark interactive surface fills (buttons, tabs, selectable rows), tinted to
    // the tube so nothing reads warm on the green screen.
    pub fn surface() -> Color {
        tube(
            Color::new(0.11, 0.085, 0.015, 1.0),
            Color::new(0.02, 0.07, 0.032, 1.0),
        )
    }
    pub fn surface_hover() -> Color {
        tube(
            Color::new(0.22, 0.16, 0.02, 1.0),
            Color::new(0.04, 0.13, 0.06, 1.0),
        )
    }
    pub fn surface_active() -> Color {
        tube(
            Color::new(0.2, 0.15, 0.02, 1.0),
            Color::new(0.04, 0.12, 0.06, 1.0),
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
    StartNewGame,
    ContinueGame,
    DeleteSave,
    // Global
    SaveGame,
    ToMenu,
    RetireVoyage,
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
    let style = SurfaceStyle::new(term::panel())
        .with_border(1.0, term::border())
        .with_header(34.0, term::panel_header())
        .with_header_divider(1.0, term::border());
    if let Some(title) = title {
        draw_surface_with_title(
            rect,
            Some(title),
            &style,
            TextStyle::new(17.0, term::primary()),
        );
    } else {
        draw_surface(
            rect,
            &SurfaceStyle::new(term::panel()).with_border(1.0, term::border()),
        );
    }
}

pub fn term_button(rect: Rect, label: &str, enabled: bool, mouse: Vec2) -> bool {
    let hovered = enabled && rect.contains_point(mouse);
    let fill = if !enabled {
        term::surface_disabled()
    } else if hovered {
        term::surface_hover()
    } else {
        term::surface()
    };
    let border = if enabled { term::dim() } else { term::faint() };
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
                term::primary()
            } else {
                term::faint()
            },
        ),
    );
    hovered && is_mouse_button_released(MouseButton::Left)
}

/// Which end of a meter is the dangerous one, for the critical-red highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterTone {
    /// A low reading is bad (hull, morale, fuel…): red under 35%.
    LowCritical,
    /// A high reading is bad (cultural drift…): red over 65%.
    HighCritical,
    /// Neither end is inherently bad (adaptation): never red.
    Neutral,
}

pub fn term_meter(rect: Rect, value: f32, max: f32, label: &str) {
    term_meter_toned(rect, value, max, label, MeterTone::LowCritical);
}

pub fn term_meter_toned(rect: Rect, value: f32, max: f32, label: &str, tone: MeterTone) {
    let frac = if max > 0.0 { value / max } else { 0.0 };
    let critical = match tone {
        MeterTone::LowCritical => frac < 0.35,
        MeterTone::HighCritical => frac > 0.65,
        MeterTone::Neutral => false,
    };
    meter(
        rect,
        value,
        max,
        if critical {
            term::alert()
        } else {
            term::accent()
        },
        Some(label),
    );
}

pub fn stat_line(x: f32, y: f32, label: &str, value: &str, value_color: Color) {
    draw_ui_text_ex(label, x, y, TextStyle::new(15.0, term::dim()).params());
    draw_text_right(value, x + 250.0, y, TextStyle::new(15.0, value_color));
}

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

pub struct MenuCtx<'a> {
    pub data: &'a GameData,
    pub menu: &'a MenuState,
    pub legacy_ids: &'a [String],
    pub chronicle: &'a ChronicleStore,
    pub ui: &'a VirtualUi,
}

pub fn draw_menu(ctx: MenuCtx<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    draw_text_glow(
        "STELLAR LEGACY",
        LOGICAL_WIDTH / 2.0 - 190.0,
        130.0,
        TextStyle::new(48.0, term::primary()),
        0.1,
        3.0,
    );
    draw_ui_text_ex(
        "// generational starship command //",
        LOGICAL_WIDTH / 2.0 - 165.0,
        165.0,
        TextStyle::new(17.0, term::dim()).params(),
    );

    // A dynasty inheriting a storied Chronicle begins with a head start (§7).
    let heritage = crate::heritage::derive(ctx.chronicle, &ctx.data.config.heritage);
    if heritage.has_bonus() {
        draw_text_centered(
            &format!(
                "HERITAGE: {} · renown {} · +{} cr / +{} inf / +{} tradition",
                heritage.tier_name,
                heritage.renown,
                heritage.credits,
                heritage.influence,
                heritage.tradition
            ),
            LOGICAL_WIDTH / 2.0,
            193.0,
            TextStyle::new(14.0, term::accent()),
        );
    }

    let panel = Rect::new(LOGICAL_WIDTH / 2.0 - 320.0, 210.0, 640.0, 420.0);
    term_panel(panel, Some("FOUNDING CHARTER"));
    let content = panel.inset(24.0);
    let mut y = content.y + 40.0;

    draw_ui_text_ex(
        "Choose the legacy that will steer your bloodline:",
        content.x,
        y,
        TextStyle::new(16.0, term::dim()).params(),
    );
    y += 18.0;

    for (i, legacy_id) in ctx.legacy_ids.iter().enumerate() {
        let Some(legacy) = ctx.data.legacies.get(legacy_id) else {
            continue;
        };
        let rect = Rect::new(content.x, y + 8.0, content.w, 62.0);
        let selected = i == ctx.menu.selected_legacy;
        let fill = if selected {
            term::surface_active()
        } else {
            term::surface_inset()
        };
        draw_surface(
            rect,
            &SurfaceStyle::new(fill).with_border(
                1.0,
                if selected {
                    term::primary()
                } else {
                    term::faint()
                },
            ),
        );
        draw_ui_text_ex(
            &legacy.name,
            rect.x + 14.0,
            rect.y + 24.0,
            TextStyle::new(
                18.0,
                if selected {
                    term::accent()
                } else {
                    term::primary()
                },
            )
            .params(),
        );
        draw_text_block(
            &legacy.description,
            rect.x + 14.0,
            rect.y + 32.0,
            rect.w - 28.0,
            26.0,
            13.0,
            2.0,
            term::dim(),
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
    /// Seconds since the current blocking modal appeared, for the terminal
    /// typewriter reveal. Large/instant when the effect is disabled.
    pub modal_reveal: f32,
    /// Seconds since the newest ship's-log entry appeared, so it streams in
    /// like live console output. Large/instant in capture.
    pub log_reveal: f32,
}

pub fn draw_gameplay(ctx: GameplayCtx<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    // Extinction halts the voyage: a full-screen terminal takeover replaces the
    // normal screens (GDD §7).
    if ctx.sim.dynasty.extinct {
        game_over::draw(&ctx, mouse, &mut actions);
        return actions;
    }

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
    draw_text_glow(
        &ctx.data.config.display_name.to_uppercase(),
        rect.x + 16.0,
        rect.y + 36.0,
        TextStyle::new(24.0, term::primary()),
        0.12,
        2.0,
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
        TextStyle::new(16.0, term::dim()).params(),
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
        TextStyle::new(15.0, term::accent()),
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
            term::surface_active()
        } else {
            term::surface_inset()
        };
        draw_surface(
            rect,
            &SurfaceStyle::new(fill).with_border(
                1.0,
                if active {
                    term::primary()
                } else {
                    term::faint()
                },
            ),
        );
        draw_text_centered_in_box_ex(
            screen.label(),
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            TextStyle::new(14.0, if active { term::accent() } else { term::dim() }),
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
