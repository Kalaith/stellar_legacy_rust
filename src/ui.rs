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
pub mod market;
pub mod prep;
pub mod settings;
pub mod ship_builder;
pub mod ship_schematic;
pub mod subsystems;
pub mod welcome;

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
    TrainSubsystemKnowledge(String),
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

/// The shared readout gauge behind every meter in the UI. A recessed track
/// holds a segment-notched fill; the `label` sits flush left and the `value`
/// flush right so neither straddles the fill edge — the old centered label was
/// unreadable exactly where it crossed the fill boundary. The label reads dark
/// over a covering fill and switches to the tube's light dim where the fill
/// falls short, so it stays legible at any level.
pub fn term_bar(rect: Rect, frac: f32, fill: Color, label: &str, value: &str) {
    let frac = frac.clamp(0.0, 1.0);
    draw_surface(
        rect,
        &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
    );
    let fill_w = (rect.w - 2.0) * frac;
    if fill_w > 0.5 {
        draw_rectangle(rect.x + 1.0, rect.y + 1.0, fill_w, rect.h - 2.0, fill);
        // Notch the filled span into even segments for the terminal-gauge read.
        // The notch is only faintly darker than the fill so it reads as a ruled
        // gauge without chopping the dark label that sits over it.
        let seg = 12.0;
        let notch = Color::new(0.0, 0.0, 0.0, 0.18);
        let mut sx = rect.x + 1.0 + seg;
        let fill_right = rect.x + 1.0 + fill_w;
        while sx < fill_right {
            draw_rectangle(sx, rect.y + 1.0, 2.0, rect.h - 2.0, notch);
            sx += seg;
        }
    }
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, term::border());

    let font = 14.0;
    let baseline = rect.y + rect.h * 0.5 + font * 0.36;
    // The label reads as near-black over a fill that covers it, and switches to
    // the bright tube color where the fill falls short so it stays legible on
    // the dark track. The char-width estimate slightly overshoots, keeping the
    // switch conservative.
    let covered = fill_w >= label.chars().count() as f32 * font * 0.62 + 12.0;
    let label_color = if covered { term::bg() } else { term::primary() };
    // A soft dark shadow keeps a bright label legible where it overhangs the
    // green fill onto the dark track (short bars like cultural drift).
    if !covered {
        draw_ui_text_ex(
            label,
            rect.x + 9.0,
            baseline + 1.0,
            TextStyle::new(font, Color::new(0.0, 0.0, 0.0, 0.55)).params(),
        );
    }
    draw_ui_text_ex(
        label,
        rect.x + 8.0,
        baseline,
        TextStyle::new(font, label_color).params(),
    );
    let value_color = if frac > 0.9 {
        term::bg()
    } else {
        term::accent()
    };
    draw_text_right(
        value,
        rect.right() - 8.0,
        baseline,
        TextStyle::new(font, value_color),
    );
}

pub fn term_meter(rect: Rect, value: f32, max: f32, label: &str, value_text: &str) {
    term_meter_toned(rect, value, max, label, value_text, MeterTone::LowCritical);
}

pub fn term_meter_toned(
    rect: Rect,
    value: f32,
    max: f32,
    label: &str,
    value_text: &str,
    tone: MeterTone,
) {
    let frac = if max > 0.0 { value / max } else { 0.0 };
    let critical = match tone {
        MeterTone::LowCritical => frac < 0.35,
        MeterTone::HighCritical => frac > 0.65,
        MeterTone::Neutral => false,
    };
    let fill = if critical {
        term::alert()
    } else {
        term::accent()
    };
    term_bar(rect, frac, fill, label, value_text);
}

pub fn stat_line(x: f32, y: f32, label: &str, value: &str, value_color: Color) {
    draw_ui_text_ex(label, x, y, TextStyle::new(15.0, term::dim()).params());
    draw_text_right(value, x + 250.0, y, TextStyle::new(15.0, value_color));
}

/// A dense spec row: dimmed label flush left, value flush right at `x + w`.
/// Used for the ship-class readout and other two-column stat blocks that want
/// the value pinned to a panel's right edge rather than a fixed column.
pub fn spec_line(x: f32, y: f32, w: f32, label: &str, value: &str, value_color: Color) {
    draw_ui_text_ex(label, x, y, TextStyle::new(14.0, term::dim()).params());
    draw_text_right(value, x + w, y, TextStyle::new(14.0, value_color));
}

/// Minimal line-art glyphs for the status gauges. Drawn from primitives (no
/// texture assets) so they tint with the active phosphor and stay crisp at any
/// scale. The labels name each gauge, so these read as distinguishing marks
/// rather than literal illustrations.
#[derive(Debug, Clone, Copy)]
pub enum GaugeIcon {
    Hull,
    Life,
    Fuel,
    Food,
    Maint,
    Alert,
    People,
}

pub fn draw_gauge_icon(icon: GaugeIcon, cx: f32, cy: f32, r: f32, color: Color) {
    let t = 1.6;
    match icon {
        GaugeIcon::Hull => draw_poly_lines(cx, cy, 6, r, 90.0, t, color),
        GaugeIcon::Life => {
            draw_circle_lines(cx, cy, r, t, color);
            draw_circle_lines(cx, cy, r * 0.42, t, color);
        }
        GaugeIcon::Fuel => draw_poly_lines(cx, cy, 4, r, 45.0, t, color),
        GaugeIcon::Food => {
            draw_line(cx, cy - r, cx, cy + r, t, color);
            for i in 0..3 {
                let yy = cy - r * 0.7 + i as f32 * r * 0.65;
                draw_line(cx, yy, cx - r * 0.6, yy + r * 0.4, t, color);
                draw_line(cx, yy, cx + r * 0.6, yy + r * 0.4, t, color);
            }
        }
        GaugeIcon::Maint => {
            draw_poly_lines(cx, cy, 8, r, 22.5, t, color);
            draw_circle_lines(cx, cy, r * 0.4, t, color);
        }
        GaugeIcon::Alert => {
            draw_triangle_lines(
                vec2(cx, cy - r),
                vec2(cx - r, cy + r * 0.85),
                vec2(cx + r, cy + r * 0.85),
                t,
                color,
            );
            draw_line(cx, cy - r * 0.25, cx, cy + r * 0.35, t, color);
            draw_circle(cx, cy + r * 0.62, t * 0.9, color);
        }
        GaugeIcon::People => {
            draw_circle_lines(cx, cy - r * 0.4, r * 0.42, t, color);
            draw_line(
                cx - r * 0.8,
                cy + r * 0.85,
                cx + r * 0.8,
                cy + r * 0.85,
                t,
                color,
            );
            draw_line(
                cx - r * 0.8,
                cy + r * 0.85,
                cx - r * 0.5,
                cy + r * 0.15,
                t,
                color,
            );
            draw_line(
                cx + r * 0.8,
                cy + r * 0.85,
                cx + r * 0.5,
                cy + r * 0.15,
                t,
                color,
            );
        }
    }
}

/// A system-status cell: a ringed line-art icon at the left with a dimmed label
/// over a bright value to its right — the mockup's bottom instrument strip and
/// the dashboard demographic tiles are rows of these.
pub fn status_badge(rect: Rect, icon: GaugeIcon, label: &str, value: &str, tone: Color) {
    let cy = rect.y + rect.h * 0.5;
    let r = (rect.h * 0.5 - 7.0).clamp(10.0, 17.0);
    let cx = rect.x + r + 6.0;
    draw_circle_lines(cx, cy, r + 5.0, 1.0, term::faint());
    draw_gauge_icon(icon, cx, cy, r * 0.72, tone);
    let tx = cx + r + 16.0;
    draw_ui_text_ex(
        label,
        tx,
        cy - 3.0,
        TextStyle::new(12.0, term::dim()).params(),
    );
    draw_ui_text_ex(value, tx, cy + 15.0, TextStyle::new(15.0, tone).params());
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
    match ctx.menu.phase {
        crate::state::MenuPhase::Main => draw_main_menu(&ctx),
        crate::state::MenuPhase::NewGame => draw_new_game(&ctx),
    }
}

/// The title / main-menu screen (real-time loop follow-up): the game title over
/// four options — CONTINUE, NEW GAME, SETTINGS, EXIT. The new-game picker is one
/// step in from here (NEW GAME).
fn draw_main_menu(ctx: &MenuCtx<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = ctx.ui.mouse_position();

    draw_text_glow(
        "STELLAR LEGACY",
        LOGICAL_WIDTH / 2.0 - 230.0,
        250.0,
        TextStyle::new(58.0, term::primary()),
        0.1,
        3.0,
    );
    draw_text_centered(
        "// generational starship command //",
        LOGICAL_WIDTH / 2.0,
        295.0,
        TextStyle::new(18.0, term::dim()),
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
            325.0,
            TextStyle::new(14.0, term::accent()),
        );
    }

    // A single centered column of options, most-common action first.
    let bw = 300.0;
    let bh = 46.0;
    let gap = 12.0;
    let bx = LOGICAL_WIDTH / 2.0 - bw / 2.0;
    let mut by = 370.0;
    if term_button(
        Rect::new(bx, by, bw, bh),
        "CONTINUE",
        ctx.menu.save_exists,
        mouse,
    ) {
        actions.push(UiAction::ContinueGame);
    }
    by += bh + gap;
    if term_button(Rect::new(bx, by, bw, bh), "NEW GAME", true, mouse) {
        actions.push(UiAction::GoToNewGame);
    }
    by += bh + gap;
    if term_button(Rect::new(bx, by, bw, bh), "SETTINGS", true, mouse) {
        actions.push(UiAction::OpenSettings);
    }
    by += bh + gap;
    if term_button(Rect::new(bx, by, bw, bh), "EXIT GAME", true, mouse) {
        actions.push(UiAction::ExitGame);
    }

    actions
}

/// The new-game screen (W7): legacy + founding-faction selection, reached from
/// the main menu via NEW GAME. BEGIN VOYAGE launches; BACK returns to the menu.
fn draw_new_game(ctx: &MenuCtx<'_>) -> Vec<UiAction> {
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

    let starting = ctx.data.config.factions.starting_count as usize;
    let tut = &ctx.data.config.tutorial;
    let panel = Rect::new(LOGICAL_WIDTH / 2.0 - 430.0, 198.0, 860.0, 508.0);
    term_panel(panel, Some("FOUNDING CHARTER"));
    let content = panel.inset(24.0);
    let col_gap = 24.0;
    let col_w = (content.w - col_gap) / 2.0;
    let left_x = content.x;
    let right_x = content.x + col_w + col_gap;
    // Both columns share a header band: an intro block explaining the choice,
    // then the pickable list below it. `list_top` is where each list starts.
    let list_top = content.y + 92.0;

    // --- Left column: the legacy that steers the bloodline ---
    draw_text_block(
        &tut.legacy_intro,
        left_x,
        content.y + 28.0,
        col_w,
        58.0,
        14.0,
        3.0,
        term::dim(),
    );
    let mut y = list_top;
    for (i, legacy_id) in ctx.legacy_ids.iter().enumerate() {
        let Some(legacy) = ctx.data.legacies.get(legacy_id) else {
            continue;
        };
        let rect = Rect::new(left_x, y + 8.0, col_w, 62.0);
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
            &format!("{} {}", i + 1, legacy.name),
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

    // A "what this changes" callout for the focused legacy, so the pick reads as
    // a mechanical choice and not just flavor. Text is the legacy's own `effects`.
    if let Some(legacy) = ctx
        .legacy_ids
        .get(ctx.menu.selected_legacy)
        .and_then(|id| ctx.data.legacies.get(id))
    {
        let callout = Rect::new(left_x, y + 6.0, col_w, content.bottom() - (y + 6.0) - 56.0);
        draw_surface(
            callout,
            &SurfaceStyle::new(term::surface_inset()).with_border(1.0, term::faint()),
        );
        draw_ui_text_ex(
            "WHAT THIS CHANGES",
            callout.x + 14.0,
            callout.y + 22.0,
            TextStyle::new(13.0, term::accent()).params(),
        );
        draw_text_block(
            &legacy.effects,
            callout.x + 14.0,
            callout.y + 30.0,
            callout.w - 28.0,
            callout.h - 40.0,
            13.0,
            3.0,
            term::primary(),
        );
    }

    // --- Right column: the founding peoples (W7) — pick exactly `starting` ---
    let chosen = ctx.menu.selected_factions.len();
    draw_text_block(
        &tut.factions_intro,
        right_x,
        content.y + 28.0,
        col_w,
        50.0,
        14.0,
        3.0,
        term::dim(),
    );
    draw_ui_text_ex(
        &format!("Choose {starting} founding peoples  ({chosen}/{starting}):"),
        right_x,
        content.y + 84.0,
        TextStyle::new(
            14.0,
            if chosen == starting {
                term::accent()
            } else {
                term::primary()
            },
        )
        .params(),
    );
    let mut fy = list_top;
    for id in GameData::sorted_ids(&ctx.data.factions) {
        let Some(faction) = ctx.data.factions.get(&id) else {
            continue;
        };
        let selected = ctx.menu.selected_factions.iter().any(|f| f == &id);
        let rect = Rect::new(right_x, fy + 4.0, col_w, 44.0);
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
            &format!("{} {}", if selected { "[x]" } else { "[ ]" }, faction.name),
            rect.x + 10.0,
            rect.y + 18.0,
            TextStyle::new(
                14.0,
                if selected {
                    term::accent()
                } else {
                    term::primary()
                },
            )
            .params(),
        );
        draw_ui_text_ex(
            faction_ideology_label(faction.ideology),
            rect.x + 10.0,
            rect.y + 36.0,
            TextStyle::new(11.0, term::dim()).params(),
        );
        if rect.contains_point(mouse) && is_mouse_button_released(MouseButton::Left) {
            actions.push(UiAction::ToggleFaction(id.clone()));
        }
        fy += 50.0;
    }

    // --- Bottom button row (spans both columns) ---
    let by = content.bottom() - 44.0;
    let btn_w = (content.w - 20.0) / 3.0;
    if term_button(
        Rect::new(content.x, by, btn_w, 44.0),
        "BEGIN VOYAGE [ENTER]",
        chosen == starting,
        mouse,
    ) {
        actions.push(UiAction::StartNewGame);
    }
    if term_button(
        Rect::new(content.x + btn_w + 10.0, by, btn_w, 44.0),
        "BACK [ESC]",
        true,
        mouse,
    ) {
        actions.push(UiAction::BackToMainMenu);
    }
    if term_button(
        Rect::new(content.x + (btn_w + 10.0) * 2.0, by, btn_w, 44.0),
        "DELETE SAVE",
        ctx.menu.save_exists,
        mouse,
    ) {
        actions.push(UiAction::DeleteSave);
    }

    actions
}

/// A short tech-spectrum tag for a faction's ideology (W7 picker flavor).
fn faction_ideology_label(ideology: f32) -> &'static str {
    if ideology > 0.66 {
        "tech-embracing · radical"
    } else if ideology > 0.2 {
        "tech-embracing"
    } else if ideology >= -0.2 {
        "pragmatic middle"
    } else if ideology >= -0.66 {
        "tech-averse"
    } else {
        "tech-averse · traditional"
    }
}

// ---------------------------------------------------------------------------
// Gameplay shell: header, tabs, screen dispatch, event modal
// ---------------------------------------------------------------------------

pub struct GameplayCtx<'a> {
    pub data: &'a GameData,
    pub sim: &'a SimState,
    pub screen: Screen,
    pub chronicle: &'a ChronicleStore,
    pub achievements: &'a Achievements,
    pub ui: &'a VirtualUi,
    /// Seconds since the current blocking modal appeared, for the terminal
    /// typewriter reveal. Large/instant when the effect is disabled.
    pub modal_reveal: f32,
    /// Seconds since the newest ship's-log entry appeared, so it streams in
    /// like live console output. Large/instant in capture.
    pub log_reveal: f32,
    /// Cosmetic wall-clock run timer (PLAN M4.7): elapsed real seconds for the
    /// current mission (live), or the last mission's while in port. `None`
    /// before the first charter. Never feeds the deterministic sim.
    pub run_clock: Option<f32>,
    /// Real seconds left before a blocking council decision auto-resolves to a
    /// random option (real-time loop §2). Only meaningful while a decision is
    /// pending; the modal renders it as a countdown.
    pub decision_remaining: f32,
    /// Smooth-scroll state for the charter board / PREP swap column (the list
    /// outgrows its panel). A `Cell` so this pure-view path can update the offset
    /// through the shared `&GameplayCtx` without threading `&mut` everywhere.
    pub charter_scroll: &'a std::cell::Cell<macroquad_toolkit::ui::ScrollArea>,
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

    // Fall back to the dashboard if the open tab is not in the current voyage
    // state's set (real-time loop §5) — e.g. an old save resuming on CONTRACT
    // while docked, before the launch/dock clamps take effect.
    let in_port = ctx.sim.contract.is_none();
    let screen = if Screen::tabs(in_port).contains(&ctx.screen) {
        ctx.screen
    } else {
        Screen::Dashboard
    };

    let content = Rect::new(16.0, 128.0, LOGICAL_WIDTH - 32.0, LOGICAL_HEIGHT - 144.0);
    match screen {
        Screen::Dashboard => dashboard::draw(&ctx, content, mouse, &mut actions),
        Screen::Drydock => contract_systems::draw_drydock(&ctx, content, mouse, &mut actions),
        Screen::ShipBuilder => ship_builder::draw(&ctx, content, mouse, &mut actions),
        Screen::Subsystems => subsystems::draw(&ctx, content, mouse, &mut actions),
        Screen::CrewDynasty => crew_dynasty::draw(&ctx, content, mouse, &mut actions),
        Screen::Contract => {
            contract_systems::draw_active_screen(&ctx, content, mouse, &mut actions)
        }
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
    // A live run timer while a mission is underway — the pacing gauge for the
    // ~30-min floor / ~1-hr cap (PLAN M4.7).
    let run_seg = if sim.contract.is_some() {
        ctx.run_clock
            .map(|secs| format!("  |  RUN {}", format_mmss(secs)))
            .unwrap_or_default()
    } else {
        String::new()
    };
    draw_ui_text_ex(
        &format!(
            "Y{:03} · M{:02}  |  GEN {}  |  {}  |  {}{}",
            sim.year(),
            sim.month(),
            sim.dynasty.generation,
            legacy,
            leader,
            run_seg
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
    // The tab set changes with voyage state (real-time loop §5): DRYDOCK + MARKET
    // in port, CONTRACT under way.
    let tabs = Screen::tabs(ctx.sim.contract.is_none());
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
        // Numbered like terminal menu entries — the digit is also the hotkey.
        draw_text_centered_in_box_ex(
            &format!("{} {}", i + 1, screen.label()),
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
