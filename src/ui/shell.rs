//! The gameplay shell: the header, the tab strip, and the per-frame
//! dispatch into whichever screen module is showing.

use super::*;

pub struct GameplayCtx<'a> {
    pub data: &'a GameData,
    pub sim: &'a SimState,
    pub screen: Screen,
    pub chronicle: &'a ChronicleStore,
    pub achievements: &'a Achievements,
    /// This frame's pointer, in logical coordinates — a mouse or a finger,
    /// asked the same way. Built once in `game.rs` so every control on the
    /// screen agrees about where it is and whether it just let go.
    pub pointer: Pointer,
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
    /// Smooth-scroll state for the CREW dynasty roster, so a dynasty larger than
    /// the panel can be read rather than counted.
    pub roster_scroll: &'a std::cell::Cell<macroquad_toolkit::ui::ScrollArea>,
    /// Smooth-scroll state for the CHRONICLE log, which accumulates across
    /// playthroughs and outlives any single save.
    pub chronicle_scroll: &'a std::cell::Cell<macroquad_toolkit::ui::ScrollArea>,
    /// SHIP builder sub-tab: `false` = LOADOUT catalog, `true` = MODULES (named
    /// subsystem version ladders). Pure view state, flipped by the on-screen toggle.
    pub ship_modules_tab: &'a std::cell::Cell<bool>,
}

pub fn draw_gameplay(ctx: GameplayCtx<'_>) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let pointer = ctx.pointer;

    // Extinction halts the voyage: a full-screen terminal takeover replaces the
    // normal screens (GDD §7).
    if ctx.sim.dynasty.extinct {
        game_over::draw(&ctx, pointer, &mut actions);
        return actions;
    }

    draw_header(&ctx);
    draw_tabs(&ctx, pointer, &mut actions);

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
        Screen::Dashboard => dashboard::draw(&ctx, content, pointer, &mut actions),
        Screen::Drydock => contract_systems::draw_drydock(&ctx, content, pointer, &mut actions),
        Screen::ShipBuilder => ship_builder::draw(&ctx, content, pointer, &mut actions),
        Screen::Subsystems => subsystems::draw(&ctx, content, pointer, &mut actions),
        Screen::CrewDynasty => crew_dynasty::draw(&ctx, content, pointer, &mut actions),
        Screen::Contract => {
            contract_systems::draw_active_screen(&ctx, content, pointer, &mut actions)
        }
        Screen::Market => market::draw(&ctx, content, pointer, &mut actions),
        Screen::Chronicle => chronicle::draw(&ctx, content, pointer, &mut actions),
    }

    // A pending council decision blocks everything else (GDD §9 step 4):
    // discard screen intents and only accept the modal's.
    if ctx.sim.pending_event.is_some() {
        actions.clear();
        event_modal::draw(&ctx, pointer, &mut actions);
    } else if ctx.sim.pending_dilemma.is_some() {
        actions.clear();
        event_modal::draw_dilemma(&ctx, pointer, &mut actions);
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

/// One chrome button (SAVE / MENU / HELP / DISPLAY) at the right of the tab row.
const CHROME_BTN_W: f32 = 88.0;
const CHROME_GAP: f32 = 6.0;
/// Width the four of them claim from the tab strip.
const CHROME_W: f32 = CHROME_BTN_W * 4.0 + CHROME_GAP * 3.0;

fn draw_tabs(ctx: &GameplayCtx<'_>, pointer: Pointer, actions: &mut Vec<UiAction>) {
    // The tab set changes with voyage state (real-time loop §5): DRYDOCK + MARKET
    // in port, CONTRACT under way.
    let tabs = Screen::tabs(ctx.sim.contract.is_none());
    let total_w = LOGICAL_WIDTH - 32.0 - CHROME_W - 12.0;
    let tab_w = (total_w - (tabs.len() as f32 - 1.0) * 6.0) / tabs.len() as f32;
    for (i, screen) in tabs.iter().enumerate() {
        let rect = Rect::new(16.0 + i as f32 * (tab_w + 6.0), 80.0, tab_w, 38.0);
        let hit = touch_area(rect);
        note_neighbour(rect);
        note_target(screen.label(), rect);
        let active = *screen == ctx.screen;
        let fill = if active || pointer.pressing(hit) {
            term::surface_active()
        } else if pointer.hovering_over(hit) {
            term::surface_hover()
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
        if !active && pointer.released_on(hit) {
            actions.push(UiAction::SelectScreen(*screen));
        }
    }

    // The chrome row: the two verbs that leave the game, and the two panels that
    // used to answer only to F1 and F2. A tablet has no function keys, so the
    // display settings — which carry the council's delegation defaults, not just
    // the CRT look — and the controls legend were simply unreachable there.
    for (i, (label, action)) in [
        ("SAVE", UiAction::SaveGame),
        ("MENU", UiAction::ToMenu),
        ("HELP", UiAction::OpenHelp),
        ("DISPLAY", UiAction::OpenSettings),
    ]
    .into_iter()
    .enumerate()
    {
        let x = LOGICAL_WIDTH - 16.0 - CHROME_W + i as f32 * (CHROME_BTN_W + CHROME_GAP);
        if term_button(Rect::new(x, 80.0, CHROME_BTN_W, 38.0), label, true, pointer) {
            actions.push(action);
        }
    }
}
