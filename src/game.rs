//! Game struct: owns the state machine, drives the tick, and composes the
//! per-frame draw. UiAction dispatch lives in [`actions`]; screenshot scene
//! seeding in [`capture_scenes`].

mod actions;
mod capture_scenes;

use crate::boot::BootScreen;
use crate::chronicle::ChronicleStore;
use crate::data::GameData;
use crate::save;
use crate::settings::DisplaySettings;
use crate::simulation::legacy;
use crate::state::{GameState, GameplayState, MenuState, SimState, StateTransition};
use crate::ui::{self, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::achievements::Achievements;
use macroquad_toolkit::assets::AssetManager;
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::fx::{CrtOverlay, CrtStyle};
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, end_virtual_ui_frame};
use macroquad_toolkit::ui::{end_frame_neighbours, Pointer, ScrollArea};
use std::cell::Cell;

/// True on the frame the number key for a 0-based list index (1..=9) is pressed.
fn digit_pressed(index: usize) -> bool {
    let key = match index {
        0 => KeyCode::Key1,
        1 => KeyCode::Key2,
        2 => KeyCode::Key3,
        3 => KeyCode::Key4,
        4 => KeyCode::Key5,
        5 => KeyCode::Key6,
        6 => KeyCode::Key7,
        7 => KeyCode::Key8,
        8 => KeyCode::Key9,
        _ => return false,
    };
    is_key_pressed(key)
}

/// A stable key identifying the blocking council decision currently up, if any
/// (`E:{template}` for an event, `D:{dilemma}` for a legacy dilemma). Drives both
/// the typewriter reveal clock and the auto-resolve countdown (real-time loop §2).
fn current_decision_key(sim: &SimState) -> Option<String> {
    if let Some(p) = &sim.pending_event {
        Some(format!("E:{}", p.template_id))
    } else {
        sim.pending_dilemma
            .as_ref()
            .map(|p| format!("D:{}", p.dilemma_id))
    }
}

pub struct Game {
    data: GameData,
    state: GameState,
    chronicle: ChronicleStore,
    /// Cross-playthrough achievements (GDD §10), persisted separately.
    achievements: Achievements,
    notifications: NotificationManager,
    events: EventBus<UiAction>,
    /// The one piece of bitmap art the game ships: the title plate behind the
    /// main menu. Everything else is drawn (GDD §0), so the manifest stays tiny.
    assets: AssetManager,
    /// Legacy ids in stable sorted order for the menu.
    legacy_ids: Vec<String>,
    /// Screen-space phosphor-monitor overlay (scanlines, vignette, flicker),
    /// drawn on top of every frame. Toggle with F10; tune via the F1 panel.
    crt: CrtOverlay,
    /// Cached overlay style derived from `display`.
    crt_style: CrtStyle,
    /// Persisted CRT display preferences.
    display: DisplaySettings,
    /// Persisted default council delegation applied to each new voyage (§5.4).
    delegation_defaults: crate::state::sim::DelegationSettings,
    /// Whether the F1 display-settings overlay is open.
    settings_open: bool,
    /// Whether the F2 help/controls overlay is open.
    help_open: bool,
    /// One-time onboarding flags (persisted separately from the campaign save).
    onboarding: crate::settings::Onboarding,
    /// Whether the first-run welcome overlay is showing (once per install).
    welcome_open: bool,
    /// Terminal typewriter reveal for blocking modals: which modal is showing
    /// and when it appeared, so its body text streams in. Purely cosmetic —
    /// never touches the deterministic sim.
    modal_key: Option<String>,
    modal_started: f64,
    /// Wall-clock `get_time()` when the current mission's charter was accepted,
    /// for the cosmetic run timer (PLAN M4.7). Session-local; never touches the
    /// deterministic sim.
    mission_started: Option<f64>,
    /// Real seconds the last completed mission took, shown in the drydock
    /// Homecoming until the next charter is accepted.
    last_mission_real_secs: Option<f32>,
    /// Capture-only override so the run timer is deterministic in screenshots.
    capture_run_secs: Option<f32>,
    /// Ship's-log stream clock: last-seen entry count and when it last grew,
    /// so the newest line types in. Cosmetic; never touches the sim.
    log_len: usize,
    log_started: f64,
    /// Reveal text instantly (screenshot capture) instead of typing it out.
    instant_reveal: bool,
    /// Capture-only: freeze the ship's-log stream at this elapsed time.
    capture_log_reveal: Option<f32>,
    /// One-shot power-on boot log shown before the menu on launch.
    boot: BootScreen,
    /// Real-time auto-advance accumulator (real-time loop §1): real seconds banked
    /// toward the next month tick while under way. Reset whenever time is not
    /// advancing (docked, paused, blocked).
    month_accumulator: f32,
    /// The blocking decision the auto-resolve countdown is currently timing
    /// (`E:{id}` / `D:{id}`), and the wall-clock it started at (real-time loop §2).
    /// Reset when the pending decision changes.
    decision_key: Option<String>,
    decision_started: f64,
    /// Smooth-scroll state for the drydock charter board / PREP swap column,
    /// which now outgrows its panel (grouped by objective, all tiers listed).
    /// A `Cell` so the pure-view draw path can update it through `&GameplayCtx`.
    charter_scroll: Cell<ScrollArea>,
    /// Smooth-scroll state for the SHIP builder's three catalog columns
    /// (Hull/Engine/Weapon), which overflow once a mission-reward part joins a
    /// full column. One `ScrollArea` per column.
    ship_scroll: Cell<[ScrollArea; 3]>,
    /// Smooth-scroll state for the CREW dynasty roster. The list used to be
    /// truncated to whatever fitted the panel, so a dynasty of five showed two
    /// and told you about the rest.
    roster_scroll: Cell<ScrollArea>,
    /// Smooth-scroll state for the CHRONICLE log, which grows across
    /// playthroughs and long ago outgrew the nine entries it used to show.
    chronicle_scroll: Cell<ScrollArea>,
    /// SHIP builder sub-tab: `false` = LOADOUT (hull/engine/weapon catalog),
    /// `true` = MODULES (the six subsystems' named version ladders). Pure view
    /// state, flipped by the on-screen toggle.
    ship_modules_tab: Cell<bool>,
}

impl Game {
    pub async fn new() -> Self {
        // Seed macroquad's shared generator from the wall clock so `random_u64`
        // (the per-campaign seed source) actually varies launch to launch — left
        // unseeded it starts from a fixed default, so every new game came out the
        // same. A `fixed_seed` in game_config still overrides this for testing.
        macroquad_toolkit::rng::srand((macroquad::miniquad::date::now() * 1000.0) as u64);
        let data = GameData::load()
            .unwrap_or_else(|err| panic!("Stellar Legacy embedded data failed to load: {err}"));
        let chronicle = ChronicleStore::load(
            &data.config.game_name,
            &data.config.chronicle_slot,
            &data.config.version,
        );
        // Menu display order: Preservers first — the founders' path reads as
        // the intuitive default — then the rest in stable sorted-id order.
        // Purely cosmetic: legacy choice is the player's, never RNG-driven.
        let mut legacy_ids = GameData::sorted_ids(&data.legacies);
        legacy_ids.sort_by_key(|id| match id.as_str() {
            "preservers" => 0,
            "adaptors" => 1,
            "wanderers" => 2,
            _ => 3,
        });
        let save_exists = save::save_exists(&data.config);
        let display = DisplaySettings::load(&data.config.game_name);
        let crt_style = display.crt_style();
        ui::term::set_phosphor(display.phosphor);
        let delegation_defaults = crate::settings::load_delegation(&data.config.game_name);

        let mut assets = AssetManager::new();
        let _ = assets.load_asset_pack("assets.zip").await;
        let _ = assets.load_texture_configs(&data.texture_manifest).await;

        let achievements = crate::achievements::load(&data.config.game_name);
        // The first-run welcome overlay is triggered by the first NEW GAME (see
        // GoToNewGame), not on boot — the main menu shows clean. The flag decides
        // whether that first NEW GAME opens it.
        let onboarding = crate::settings::Onboarding::load(&data.config.game_name);

        Self {
            data,
            state: GameState::Menu(MenuState::new(save_exists)),
            chronicle,
            achievements,
            notifications: NotificationManager::new(),
            events: EventBus::new(),
            assets,
            legacy_ids,
            crt: CrtOverlay::new(),
            crt_style,
            display,
            delegation_defaults,
            settings_open: false,
            help_open: false,
            onboarding,
            welcome_open: false,
            modal_key: None,
            modal_started: 0.0,
            mission_started: None,
            last_mission_real_secs: None,
            capture_run_secs: None,
            log_len: 0,
            log_started: 0.0,
            instant_reveal: false,
            capture_log_reveal: None,
            boot: BootScreen::new(),
            month_accumulator: 0.0,
            decision_key: None,
            decision_started: 0.0,
            charter_scroll: Cell::new(ScrollArea::new()),
            ship_scroll: Cell::new([ScrollArea::new(); 3]),
            roster_scroll: Cell::new(ScrollArea::new()),
            chronicle_scroll: Cell::new(ScrollArea::new()),
            ship_modules_tab: Cell::new(false),
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.notifications.update(dt);

        // First-run welcome overlay sits above the menu once the boot log ends.
        // Any key dismisses it (its button is handled in draw); until then, no
        // other input runs and any menu intents queued beneath it are dropped.
        if self.boot.is_done() && self.welcome_open {
            if get_last_key_pressed().is_some() {
                self.dismiss_welcome();
            }
            let _ = self.events.drain();
            return;
        }

        // F10 toggles the CRT effect outright; F1 opens the display panel.
        if is_key_pressed(KeyCode::F10) {
            self.display.crt_enabled = !self.display.crt_enabled;
            self.persist_display();
        }
        if self.boot.is_done() && is_key_pressed(KeyCode::F1) {
            self.settings_open = !self.settings_open;
            self.help_open = false;
        }
        if self.boot.is_done() && is_key_pressed(KeyCode::F2) {
            self.help_open = !self.help_open;
            self.settings_open = false;
        }
        // Esc closes whichever panel is open (help first, then settings).
        if is_key_pressed(KeyCode::Escape) {
            if self.help_open {
                self.help_open = false;
            } else if self.settings_open {
                self.settings_open = false;
            }
        }

        // Boot log plays once before the menu; any input skips it. Capture mode
        // freezes it at a seeked frame (instant_reveal), so don't advance then.
        if !self.boot.is_done() && matches!(self.state, GameState::Menu(_)) {
            if !self.instant_reveal {
                self.boot.update(dt);
                if is_mouse_button_pressed(MouseButton::Left) || get_last_key_pressed().is_some() {
                    self.boot.finish();
                }
            }
            return;
        }

        let mut actions: Vec<UiAction> = self.events.drain().collect();
        self.gather_keyboard_actions(&mut actions);
        let mut transition = None;
        for action in actions {
            if let Some(t) = self.apply_action(action) {
                transition = Some(t);
            }
        }
        if let Some(transition) = transition {
            self.transition(transition);
        }

        // Drive real time last, once input has been applied (real-time loop §1/§2):
        // auto-advance the month clock under way, or run the decision countdown.
        self.update_realtime(dt);
    }

    /// The real-time driver (real-time loop §1/§2). While under way and unpaused,
    /// bank real seconds toward the next month and step the tick each time the
    /// per-month threshold is crossed, hard-stopping the moment a decision,
    /// completion, or extinction lands. While a decision blocks, freeze the clock
    /// and auto-resolve it once the countdown runs out. Docked, nothing advances.
    /// Skipped entirely in capture mode (deterministic screenshots).
    fn update_realtime(&mut self, dt: f32) {
        if self.instant_reveal {
            return;
        }
        let (is_gameplay, key, can_advance, multiplier) = match &self.state {
            GameState::Gameplay(g) => {
                let key = current_decision_key(&g.sim);
                let can_advance = key.is_none()
                    && !g.sim.dynasty.extinct
                    && g.sim.contract.is_some()
                    && g.sim.speed != crate::state::sim::GameSpeed::Paused;
                (true, key, can_advance, g.sim.speed.multiplier())
            }
            _ => (false, None, false, 0.0),
        };
        if !is_gameplay {
            self.decision_key = None;
            self.month_accumulator = 0.0;
            return;
        }

        // Track the countdown clock, restarting it whenever the decision changes.
        if key != self.decision_key {
            self.decision_key = key.clone();
            self.decision_started = get_time();
        }

        if key.is_some() {
            // A decision blocks time; let the clock decide once it runs out.
            self.month_accumulator = 0.0;
            let timeout = self.data.config.real_time.decision_timeout_secs as f64;
            if get_time() - self.decision_started >= timeout {
                self.auto_resolve_decision();
            }
            return;
        }

        if !can_advance {
            self.month_accumulator = 0.0;
            return;
        }

        self.month_accumulator += dt * multiplier;
        let per_month = self.data.config.real_time.seconds_per_month.max(0.01);
        while self.month_accumulator >= per_month {
            self.month_accumulator -= per_month;
            self.advance_one_month();
            // Stop bursting months the instant something needs the player or the
            // voyage ended; the remainder is dropped so it doesn't fast-forward on
            // resume.
            let stop = match &self.state {
                GameState::Gameplay(g) => {
                    g.sim.has_pending_decision()
                        || g.sim.dynasty.extinct
                        || g.sim.contract.is_none()
                }
                _ => true,
            };
            if stop {
                self.month_accumulator = 0.0;
                break;
            }
        }
    }

    /// Terminal-style keyboard navigation. On the menu, number keys pick a
    /// legacy, arrows move the selection, Enter begins the voyage. In gameplay,
    /// a blocking council modal takes the number keys for its options, otherwise
    /// the number keys switch screen tabs. Time advances on its own (real-time
    /// loop §1), so there is no manual step key. Suppressed while the settings or
    /// help panel is up.
    fn gather_keyboard_actions(&mut self, actions: &mut Vec<UiAction>) {
        if self.settings_open || self.help_open {
            return;
        }

        // Menu: keyboard is only wired on the new-game picker (terminals are
        // keyboard-first); the main-menu screen is mouse-driven.
        if let GameState::Menu(menu) = &self.state {
            if menu.phase != crate::state::MenuPhase::NewGame {
                return;
            }
            let selected = menu.selected_legacy;
            let count = self.legacy_ids.len();
            for i in 0..count {
                if digit_pressed(i) {
                    actions.push(UiAction::SelectLegacy(i));
                }
            }
            if is_key_pressed(KeyCode::Up) {
                actions.push(UiAction::SelectLegacy(selected.saturating_sub(1)));
            }
            if is_key_pressed(KeyCode::Down) {
                actions.push(UiAction::SelectLegacy(
                    (selected + 1).min(count.saturating_sub(1)),
                ));
            }
            if is_key_pressed(KeyCode::Enter) {
                actions.push(UiAction::StartNewGame);
            }
            if is_key_pressed(KeyCode::Escape) {
                actions.push(UiAction::BackToMainMenu);
            }
            return;
        }

        let GameState::Gameplay(gameplay) = &self.state else {
            return;
        };
        let sim = &gameplay.sim;

        // A pending council decision claims the number keys for its choices.
        if let Some(pending) = &sim.pending_event {
            if let Some(template) = self.data.events.get(&pending.template_id) {
                for i in 0..template.outcomes.len() {
                    if digit_pressed(i) {
                        actions.push(UiAction::ResolveEvent(i));
                    }
                }
            }
            return;
        }
        if sim.pending_dilemma.is_some() {
            if let Some(dilemma) = legacy::pending_dilemma_def(sim, &self.data) {
                for i in 0..dilemma.options.len() {
                    if digit_pressed(i) {
                        actions.push(UiAction::ResolveDilemma(i));
                    }
                }
            }
            return;
        }

        // Number keys switch tabs within the current voyage state's set (real-time
        // loop §5). Time advances on its own now — there is no manual step key.
        let in_port = sim.contract.is_none();
        for (i, screen) in crate::state::Screen::tabs(in_port).iter().enumerate() {
            if digit_pressed(i) {
                actions.push(UiAction::SelectScreen(*screen));
            }
        }
    }

    pub fn draw(&mut self) {
        clear_background(ui::term::bg());

        let modal_reveal = self.modal_reveal();
        let log_reveal = self.log_reveal();

        let show_boot = !self.boot.is_done() && matches!(self.state, GameState::Menu(_));

        let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
        // One pointer for the whole frame, in logical coordinates — a mouse or a
        // finger, read the same way. Built here rather than per screen so every
        // control agrees about where it is and whether it just let go.
        let pointer = Pointer::read(|p| virtual_ui.screen_to_ui(p));
        let actions = if show_boot {
            self.boot.draw();
            Vec::new()
        } else {
            match &self.state {
                GameState::Menu(menu) => ui::draw_menu(ui::MenuCtx {
                    data: &self.data,
                    menu,
                    legacy_ids: &self.legacy_ids,
                    chronicle: &self.chronicle,
                    pointer,
                    title_art: self.assets.get_texture("title"),
                }),
                GameState::Gameplay(gameplay) => ui::draw_gameplay(ui::GameplayCtx {
                    data: &self.data,
                    sim: &gameplay.sim,
                    screen: gameplay.screen,
                    chronicle: &self.chronicle,
                    achievements: &self.achievements,
                    pointer,
                    modal_reveal,
                    log_reveal,
                    run_clock: self.run_clock_for(&gameplay.sim),
                    decision_remaining: self.decision_remaining(&gameplay.sim),
                    charter_scroll: &self.charter_scroll,
                    ship_scroll: &self.ship_scroll,
                    roster_scroll: &self.roster_scroll,
                    chronicle_scroll: &self.chronicle_scroll,
                    ship_modules_tab: &self.ship_modules_tab,
                }),
            }
        };

        // The F1/F2 panels float above everything and capture their own input.
        let display_actions = if self.settings_open {
            ui::settings::draw(&self.display, &self.delegation_defaults, pointer)
        } else {
            Vec::new()
        };
        let help_close = self.help_open && ui::help::draw(pointer);
        // First-run welcome overlay, above the menu only; its button dismisses it.
        let welcome_dismiss = self.welcome_open
            && matches!(self.state, GameState::Menu(_))
            && ui::welcome::draw(&self.data.config.welcome, pointer);
        // Roll this frame's controls into next frame's hit-area growth limits.
        // Every button grows toward the 44px touch standard only as far as its
        // neighbours allow, and the neighbours are whatever drew last frame — so
        // this has to happen once, after everything has drawn.
        end_frame_neighbours();
        end_virtual_ui_frame();

        // While a panel or the welcome overlay is open, swallow the underlying
        // screen's intents.
        if !self.settings_open && !self.help_open && !self.welcome_open {
            for action in actions {
                self.events.push(action);
            }
        }
        for action in display_actions {
            self.apply_display_action(action);
        }
        if help_close {
            self.help_open = false;
        }
        if welcome_dismiss {
            self.dismiss_welcome();
        }

        self.notifications
            .draw_with_config(&NotificationRenderConfig {
                anchor: NotificationAnchor::BottomRight,
                ..Default::default()
            });

        // Phosphor-monitor overlay sits on top of everything else.
        if self.display.crt_enabled {
            self.crt.draw(get_time() as f32, &self.crt_style);
        }
    }

    /// Re-derive the cached CRT style from the current settings and save them.
    fn persist_display(&mut self) {
        self.crt_style = self.display.crt_style();
        ui::term::set_phosphor(self.display.phosphor);
        if let Err(err) = self.display.save(&self.data.config.game_name) {
            self.notifications
                .warning(format!("Display settings not saved: {err}"));
        }
    }

    /// Close the first-run welcome overlay and remember it was seen, so it never
    /// shows again on this install.
    fn dismiss_welcome(&mut self) {
        if !self.welcome_open {
            return;
        }
        self.welcome_open = false;
        self.onboarding.welcome_seen = true;
        if let Err(err) = self.onboarding.save(&self.data.config.game_name) {
            self.notifications
                .warning(format!("Onboarding flag not saved: {err}"));
        }
    }

    /// Apply an intent from the display-settings overlay.
    fn apply_display_action(&mut self, action: crate::ui::settings::DisplayAction) {
        use crate::ui::settings::DisplayAction;
        match action {
            DisplayAction::ToggleCrt => self.display.crt_enabled = !self.display.crt_enabled,
            DisplayAction::ToggleScanlines => self.display.scanlines = !self.display.scanlines,
            DisplayAction::ToggleFlicker => self.display.flicker = !self.display.flicker,
            DisplayAction::SetPhosphor(p) => self.display.phosphor = p,
            DisplayAction::ToggleDelegationDefault(category) => {
                self.delegation_defaults.toggle(category);
                if let Err(err) = crate::settings::save_delegation(
                    &self.delegation_defaults,
                    &self.data.config.game_name,
                ) {
                    self.notifications
                        .warning(format!("Delegation defaults not saved: {err}"));
                }
                return; // no CRT re-derive needed
            }
            DisplayAction::Close => self.settings_open = false,
        }
        self.persist_display();
    }

    /// Seconds since the current blocking modal appeared, resetting the clock
    /// whenever a different modal takes over. Returns a large value (instant
    /// reveal) in capture mode; the value is unused when no modal is showing.
    fn modal_reveal(&mut self) -> f32 {
        if self.instant_reveal {
            return f32::MAX;
        }
        let key = match &self.state {
            GameState::Gameplay(g) => current_decision_key(&g.sim),
            _ => None,
        };
        if key != self.modal_key {
            self.modal_key = key;
            self.modal_started = get_time();
        }
        (get_time() - self.modal_started) as f32
    }

    /// Seconds since the newest ship's-log line appeared, resetting whenever the
    /// log grows so the latest entry streams in. Large (instant) in capture or
    /// outside gameplay.
    fn log_reveal(&mut self) -> f32 {
        if let Some(frozen) = self.capture_log_reveal {
            return frozen;
        }
        if self.instant_reveal {
            return f32::MAX;
        }
        let GameState::Gameplay(gameplay) = &self.state else {
            return f32::MAX;
        };
        let len = gameplay.sim.log.len();
        if len != self.log_len {
            self.log_len = len;
            self.log_started = get_time();
        }
        (get_time() - self.log_started) as f32
    }

    /// The cosmetic run timer's elapsed seconds (PLAN M4.7): live while a mission
    /// is active, frozen at the last mission's time while in port, and a fixed
    /// override in capture. Never feeds the deterministic sim.
    /// Real seconds left before the current blocking decision auto-resolves
    /// (real-time loop §2). Full timeout in capture (the clock never runs there);
    /// 0 when nothing is pending.
    fn decision_remaining(&self, sim: &SimState) -> f32 {
        let timeout = self.data.config.real_time.decision_timeout_secs;
        if self.instant_reveal {
            return timeout;
        }
        if !sim.has_pending_decision() {
            return 0.0;
        }
        (timeout - (get_time() - self.decision_started) as f32).max(0.0)
    }

    fn run_clock_for(&self, sim: &SimState) -> Option<f32> {
        if let Some(secs) = self.capture_run_secs {
            return Some(secs);
        }
        if sim.contract.is_some() {
            self.mission_started.map(|t| (get_time() - t) as f32)
        } else {
            self.last_mission_real_secs
        }
    }

    fn transition(&mut self, transition: StateTransition) {
        // Any state change clears the session-local run timer (PLAN M4.7).
        self.mission_started = None;
        self.last_mission_real_secs = None;
        match transition {
            StateTransition::NewCampaign {
                legacy_id,
                seed,
                faction_ids,
            } => {
                let mut sim = SimState::new_campaign(&self.data, &legacy_id, seed, &faction_ids);
                // A new dynasty inherits a head start from the Chronicle (§7)
                // and the player's default council delegation (§5.4).
                sim.delegation = self.delegation_defaults;
                let heritage = crate::heritage::derive(&self.chronicle, &self.data.config.heritage);
                crate::heritage::apply(&mut sim, &heritage);
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
                if heritage.has_bonus() {
                    self.notifications.success(format!(
                        "The {} heritage steadies the founding oath.",
                        heritage.tier_name
                    ));
                } else {
                    self.notifications
                        .success("The founding generation takes its oath.");
                }
            }
            StateTransition::LoadCampaign => match save::load_campaign(&self.data.config) {
                Ok(sim) => {
                    self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
                    self.notifications.success("Voyage resumed.");
                }
                Err(err) => self.notifications.danger(format!("Load failed: {err}")),
            },
            StateTransition::ToMenu => {
                if let GameState::Gameplay(gameplay) = &self.state {
                    match save::save_campaign(&self.data.config, &gameplay.sim) {
                        Ok(()) => self.notifications.info("Voyage autosaved."),
                        Err(err) => self.notifications.danger(format!("Autosave failed: {err}")),
                    }
                }
                self.state = GameState::Menu(MenuState::new(save::save_exists(&self.data.config)));
            }
        }
        self.check_achievements();
    }

    /// Unlock any achievements the current state satisfies, notifying once each
    /// and persisting on change. Cheap to call on any state change.
    fn check_achievements(&mut self) {
        let ids: Vec<&'static str> = match &self.state {
            GameState::Gameplay(gameplay) => {
                crate::achievements::evaluate(&gameplay.sim, &self.chronicle)
            }
            GameState::Menu(_) => return,
        };
        let mut changed = false;
        for id in ids {
            if self.achievements.unlock(id) {
                changed = true;
                if let Some(achievement) = self.achievements.get(id) {
                    self.notifications
                        .success(format!("Achievement unlocked: {}", achievement.name));
                }
            }
        }
        if changed {
            let _ = crate::achievements::save(&self.achievements, &self.data.config.game_name);
        }
    }
}
