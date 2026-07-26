//! The gameplay shell: the header, the tab strip, and the per-frame
//! dispatch into whichever screen module is showing.

use super::*;

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
    /// Smooth-scroll state for the SHIP builder's three catalog columns, so a
    /// column that overflows (e.g. a mission-reward part added to a full one)
    /// stays reachable. Indexed Hull / Engine / Weapon.
    pub ship_scroll: &'a std::cell::Cell<[macroquad_toolkit::ui::ScrollArea; 3]>,
    /// SHIP builder sub-tab: `false` = LOADOUT catalog, `true` = MODULES (named
    /// subsystem version ladders). Pure view state, flipped by the on-screen toggle.
    pub ship_modules_tab: &'a std::cell::Cell<bool>,
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
