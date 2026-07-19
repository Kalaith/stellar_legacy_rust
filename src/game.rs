//! Game struct: owns the state machine, applies UiActions, drives the tick.

use crate::boot::BootScreen;
use crate::chronicle::{ChronicleEntry, ChronicleStore};
use crate::data::ship_components::ComponentKind;
use crate::data::GameData;
use crate::save;
use crate::settings::DisplaySettings;
use crate::simulation::{contract, crew, event_resolver, legacy, market, tick};
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
use macroquad_toolkit::persistence::delete_slot;
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, end_virtual_ui_frame};
use macroquad_toolkit::rng;

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

pub struct Game {
    data: GameData,
    state: GameState,
    chronicle: ChronicleStore,
    /// Cross-playthrough achievements (GDD §10), persisted separately.
    achievements: Achievements,
    notifications: NotificationManager,
    events: EventBus<UiAction>,
    /// Kept wired for toolkit consistency; this game ships no sprite art
    /// (GDD §0) so the manifest stays empty.
    _assets: AssetManager,
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
}

impl Game {
    pub async fn new() -> Self {
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

        Self {
            data,
            state: GameState::Menu(MenuState::new(save_exists)),
            chronicle,
            achievements,
            notifications: NotificationManager::new(),
            events: EventBus::new(),
            _assets: assets,
            legacy_ids,
            crt: CrtOverlay::new(),
            crt_style,
            display,
            delegation_defaults,
            settings_open: false,
            help_open: false,
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
        }
    }

    /// Seed a deterministic state for the headless screenshot harness.
    pub fn begin_capture_scene(&mut self, scene: &str) {
        // Screenshots want the final composed frame, not a mid-type one, and
        // never the boot log. Force canonical amber display so captures are
        // deterministic regardless of any persisted preference.
        self.instant_reveal = true;
        self.capture_run_secs = None;
        self.boot.finish();
        self.display = DisplaySettings::default();
        self.crt_style = self.display.crt_style();
        ui::term::set_phosphor(self.display.phosphor);
        self.delegation_defaults = crate::state::sim::DelegationSettings::default();
        match scene {
            "menu" => self.state = GameState::Menu(MenuState::new(false)),
            "green" => {
                // Same menu on the green (P1) tube, to verify the recolor.
                self.display.phosphor = crate::settings::Phosphor::Green;
                self.crt_style = self.display.crt_style();
                ui::term::set_phosphor(self.display.phosphor);
                self.state = GameState::Menu(MenuState::new(true));
            }
            "settings" => {
                // Delegate one category so the capture shows both toggle states.
                self.delegation_defaults.mission_milestone = true;
                self.state = GameState::Menu(MenuState::new(true));
                self.settings_open = true;
            }
            "help" => {
                self.state = GameState::Menu(MenuState::new(true));
                self.help_open = true;
            }
            "heritage" => {
                // Seed a storied Chronicle so the menu heritage line shows.
                for i in 0..6 {
                    self.chronicle.record(crate::chronicle::ChronicleEntry {
                        completed_year: 60,
                        contract_name: "Founding Charter: Meridian Reach".to_owned(),
                        objective: "Colonization".to_owned(),
                        legacy_id: "preservers".to_owned(),
                        leader_name: "Boro Chartwright".to_owned(),
                        generation: i + 1,
                        score: 0.95,
                        outcome: "Complete".to_owned(),
                        duration_years: 60,
                    });
                }
                self.state = GameState::Menu(MenuState::new(true));
            }
            "boot" => {
                // Freeze the boot log mid-stream for a screenshot.
                self.boot.seek(1.4);
                self.state = GameState::Menu(MenuState::new(false));
            }
            "log" => {
                // Dashboard with the newest log line frozen mid-stream
                // (cursor-visible phase).
                self.capture_log_reveal = Some(0.5);
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                if let Some(template) = self.data.contracts.get("founding_colony") {
                    sim.contract = Some(contract::start_contract(template, &sim));
                }
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "event" => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                sim.pending_event = Some(crate::state::sim::PendingEvent {
                    template_id: "cultural_schism".to_owned(),
                    rolled_year: 0,
                });
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "crew" => {
                let sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::CrewDynasty;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "ship" => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                // Seed a salvage hold so the SALVAGE HOLD strip shows (M4.4).
                sim.ship.salvage = vec!["mass_driver".to_owned(), "solar_sail".to_owned()];
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::ShipBuilder;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "market" => {
                let sim = SimState::new_campaign(&self.data, "wanderers", 0xC0FFEE);
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::Market;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "contracts" => {
                // No active contract, so the available-charters list is shown.
                let sim = SimState::new_campaign(&self.data, "wanderers", 0xC0FFEE);
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::Contract;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "drydock" => {
                // Home from a mission (M4.6): no active contract, a worn ship,
                // and a concluded charter in the Chronicle → the Homecoming banner.
                let mut sim = SimState::new_campaign(&self.data, "wanderers", 0xC0FFEE);
                sim.ship.hull_integrity = 0.46;
                sim.ship.life_support = 0.58;
                sim.ship.spare_parts = 3;
                self.chronicle.record(crate::chronicle::ChronicleEntry {
                    completed_year: 41,
                    contract_name: "Deep Vein Survey: Karst Belt".to_owned(),
                    objective: "Mining".to_owned(),
                    legacy_id: "wanderers".to_owned(),
                    leader_name: "Sella Voss".to_owned(),
                    generation: 2,
                    score: 0.82,
                    outcome: "Partial".to_owned(),
                    duration_years: 40,
                });
                self.capture_run_secs = Some(2280.0); // 38m — the run just flown
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::Contract;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "contract_active" => {
                // A charter a dozen years in, to show progress + drive assist.
                let mut sim = SimState::new_campaign(&self.data, "adaptors", 0xC0FFEE);
                if let Some(template) = self.data.contracts.get("deep_vein_survey") {
                    sim.contract = Some(contract::start_contract(template, &sim));
                }
                sim.resources.food = 1_000_000;
                for _ in 0..12 {
                    sim.pending_event = None;
                    sim.pending_dilemma = None;
                    tick::advance_year(&mut sim, &self.data);
                }
                sim.pending_event = None;
                sim.pending_dilemma = None;
                self.capture_run_secs = Some(1140.0); // 19m into the run (live timer)
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::Contract;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "dilemma" => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                sim.pending_dilemma = Some(crate::state::sim::PendingDilemma {
                    dilemma_id: "archive_purge".to_owned(),
                    rolled_year: 0,
                });
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "dilemma_combat" => {
                // Wanderer convoy raid with a weapon installed — combat lifts
                // the shown odds.
                let mut sim = SimState::new_campaign(&self.data, "wanderers", 0xC0FFEE);
                sim.ship.weapon = Some("mass_driver".to_owned());
                sim.pending_dilemma = Some(crate::state::sim::PendingDilemma {
                    dilemma_id: "convoy_raid".to_owned(),
                    rolled_year: 0,
                });
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "chronicle" => {
                // Seed a storied Chronicle and unlock the matching milestones.
                self.achievements =
                    Achievements::from_definitions(crate::achievements::definitions());
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                sim.dynasty.generation = 5;
                sim.year = 120;
                for i in 0..5 {
                    self.chronicle.record(crate::chronicle::ChronicleEntry {
                        completed_year: 40 + i * 20,
                        contract_name: "Deep Vein Survey: Karst Belt".to_owned(),
                        objective: "Mining".to_owned(),
                        legacy_id: "preservers".to_owned(),
                        leader_name: "Boro Chartwright".to_owned(),
                        generation: i + 1,
                        score: 0.92,
                        outcome: if i % 2 == 0 { "Complete" } else { "Partial" }.to_owned(),
                        duration_years: 40,
                    });
                }
                for id in crate::achievements::evaluate(&sim, &self.chronicle) {
                    self.achievements.unlock(id);
                }
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::Chronicle;
                self.state = GameState::Gameplay(Box::new(gameplay));
            }
            "gameover" => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                sim.year = 148;
                sim.dynasty.generation = 6;
                sim.legacy.tradition_points = 210;
                sim.dynasty.extinct = true;
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            // "gameplay" and anything else: a fresh campaign on the dashboard.
            _ => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                if let Some(template) = self.data.contracts.get("founding_colony") {
                    sim.contract = Some(contract::start_contract(template, &sim));
                }
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.notifications.update(dt);

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
    }

    /// Terminal-style keyboard navigation. On the menu, number keys pick a
    /// legacy, arrows move the selection, Enter begins the voyage. In gameplay,
    /// a blocking council modal takes the number keys for its options, otherwise
    /// 1-6 switch screen tabs and Space/Enter advances the year. Suppressed while
    /// the settings or help panel is up.
    fn gather_keyboard_actions(&mut self, actions: &mut Vec<UiAction>) {
        if self.settings_open || self.help_open {
            return;
        }

        // Menu: keyboard legacy selection and launch (terminals are keyboard-first).
        if let GameState::Menu(menu) = &self.state {
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

        for (i, screen) in crate::state::Screen::ALL.iter().enumerate() {
            if digit_pressed(i) {
                actions.push(UiAction::SelectScreen(*screen));
            }
        }
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
            actions.push(UiAction::AdvanceYear);
        }
    }

    pub fn draw(&mut self) {
        clear_background(ui::term::bg());

        let modal_reveal = self.modal_reveal();
        let log_reveal = self.log_reveal();

        let show_boot = !self.boot.is_done() && matches!(self.state, GameState::Menu(_));

        let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
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
                    ui: &virtual_ui,
                }),
                GameState::Gameplay(gameplay) => ui::draw_gameplay(ui::GameplayCtx {
                    data: &self.data,
                    sim: &gameplay.sim,
                    screen: gameplay.screen,
                    chronicle: &self.chronicle,
                    achievements: &self.achievements,
                    ui: &virtual_ui,
                    modal_reveal,
                    log_reveal,
                    run_clock: self.run_clock_for(&gameplay.sim),
                }),
            }
        };

        // The F1/F2 panels float above everything and capture their own input.
        let display_actions = if self.settings_open {
            ui::settings::draw(
                &self.display,
                &self.delegation_defaults,
                virtual_ui.mouse_position(),
            )
        } else {
            Vec::new()
        };
        let help_close = self.help_open && ui::help::draw(virtual_ui.mouse_position());
        end_virtual_ui_frame();

        // While a panel is open, swallow the underlying screen's intents.
        if !self.settings_open && !self.help_open {
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
            GameState::Gameplay(g) => {
                if let Some(p) = &g.sim.pending_event {
                    Some(format!("E:{}", p.template_id))
                } else {
                    g.sim
                        .pending_dilemma
                        .as_ref()
                        .map(|p| format!("D:{}", p.dilemma_id))
                }
            }
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
            StateTransition::NewCampaign { legacy_id, seed } => {
                let mut sim = SimState::new_campaign(&self.data, &legacy_id, seed);
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

    fn apply_action(&mut self, action: UiAction) -> Option<StateTransition> {
        match action {
            // ---- Menu ----
            UiAction::SelectLegacy(index) => {
                if let GameState::Menu(menu) = &mut self.state {
                    menu.selected_legacy = index.min(self.legacy_ids.len().saturating_sub(1));
                }
                None
            }
            UiAction::StartNewGame => {
                let legacy_id = match &self.state {
                    GameState::Menu(menu) => self
                        .legacy_ids
                        .get(menu.selected_legacy)
                        .cloned()
                        .unwrap_or_else(|| "preservers".to_owned()),
                    _ => "preservers".to_owned(),
                };
                // Seed is random per campaign; determinism holds *within* a
                // campaign because the seed is stored in the save (GDD §5.6).
                Some(StateTransition::NewCampaign {
                    legacy_id,
                    seed: rng::random_u64(),
                })
            }
            UiAction::ContinueGame => Some(StateTransition::LoadCampaign),
            UiAction::DeleteSave => {
                match delete_slot(&self.data.config.game_name, &self.data.config.save_slot) {
                    Ok(()) => self.notifications.info("Save slot cleared."),
                    Err(err) => self.notifications.danger(format!("Delete failed: {err}")),
                }
                if let GameState::Menu(menu) = &mut self.state {
                    menu.save_exists = save::save_exists(&self.data.config);
                }
                None
            }

            // ---- Global ----
            UiAction::SaveGame => {
                if let GameState::Gameplay(gameplay) = &self.state {
                    match save::save_campaign(&self.data.config, &gameplay.sim) {
                        Ok(()) => self.notifications.success("Voyage saved."),
                        Err(err) => self.notifications.danger(format!("Save failed: {err}")),
                    }
                }
                None
            }
            UiAction::ToMenu => Some(StateTransition::ToMenu),
            UiAction::RetireVoyage => {
                // Clear the dead campaign so it can't be resumed (no autosave),
                // then return to the menu. The Chronicle persists separately.
                if let Err(err) =
                    delete_slot(&self.data.config.game_name, &self.data.config.save_slot)
                {
                    self.notifications
                        .warning(format!("Save clear failed: {err}"));
                }
                self.state = GameState::Menu(MenuState::new(save::save_exists(&self.data.config)));
                self.mission_started = None;
                self.last_mission_real_secs = None;
                self.notifications
                    .info("Voyage retired. The Chronicle remembers.");
                None
            }
            UiAction::SelectScreen(screen) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    gameplay.screen = screen;
                }
                None
            }

            // ---- Gameplay ----
            UiAction::AdvanceYear => {
                self.advance_year();
                self.check_achievements();
                None
            }
            UiAction::ResolveEvent(index) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    let sim = &mut gameplay.sim;
                    let template = sim
                        .pending_event
                        .as_ref()
                        .and_then(|p| self.data.events.get(&p.template_id))
                        .cloned();
                    if let Some(template) = template {
                        event_resolver::apply_outcome(sim, &template, index);
                    } else {
                        sim.pending_event = None;
                    }
                }
                None
            }
            UiAction::ResolveDilemma(index) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    legacy::resolve_dilemma(&mut gameplay.sim, &self.data, index);
                }
                None
            }
            UiAction::RecruitCrew(archetype_id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crew::recruit(&mut gameplay.sim, &self.data, &archetype_id) {
                        Ok(name) => self.notifications.success(format!("{name} signed on.")),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::TrainCrew(archetype_id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crew::train(&mut gameplay.sim, &self.data, &archetype_id) {
                        Ok(name) => self
                            .notifications
                            .success(format!("{name} completed training.")),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::SelectHeir(member_id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    let sim = &mut gameplay.sim;
                    let name = sim
                        .dynasty
                        .members
                        .iter()
                        .find(|m| m.id == member_id && !m.is_leader)
                        .map(|m| m.name.clone());
                    if let Some(name) = name {
                        sim.dynasty.designated_heir = Some(member_id);
                        sim.push_log(format!("The council named {name} heir designate."));
                        self.notifications.success(format!("{name} named heir."));
                    }
                }
                None
            }
            UiAction::AcceptContract(id) => {
                let mut accepted = false;
                if let (GameState::Gameplay(gameplay), Some(template)) =
                    (&mut self.state, self.data.contracts.get(&id))
                {
                    let sim = &mut gameplay.sim;
                    if sim.contract.is_none() {
                        sim.contract = Some(contract::start_contract(template, sim));
                        sim.push_log(format!("Charter accepted: {}", template.name));
                        accepted = true;
                    }
                }
                if accepted {
                    // Start the cosmetic run timer for this mission (PLAN M4.7).
                    self.mission_started = Some(get_time());
                    self.last_mission_real_secs = None;
                    self.notifications.success("Charter accepted.");
                }
                None
            }
            UiAction::PurchaseComponent(kind, id) => {
                self.purchase_component(kind, &id);
                None
            }
            UiAction::FieldRepair(kind) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::field_repair(
                        &mut gameplay.sim,
                        &self.data.config,
                        kind,
                    ) {
                        Ok(()) => self.notifications.success("Field repair complete."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::FullRepair => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::full_repair(&mut gameplay.sim, &self.data.config)
                    {
                        Ok(()) => self.notifications.success("Full refit complete."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::InstallSalvage(id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::install_salvage(
                        &mut gameplay.sim,
                        &self.data,
                        &id,
                    ) {
                        Ok(()) => self.notifications.success("Salvage installed."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::CommissionShip(id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::commission_ship(
                        &mut gameplay.sim,
                        &self.data,
                        &id,
                    ) {
                        Ok(()) => self.notifications.success("New ship commissioned."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::Buy(resource, amount) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match market::buy(&mut gameplay.sim, resource, amount) {
                        Ok(()) => self
                            .notifications
                            .success(format!("Bought {amount} {}", resource.label())),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::Sell(resource, amount) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match market::sell(&mut gameplay.sim, resource, amount) {
                        Ok(()) => self
                            .notifications
                            .success(format!("Sold {amount} {}", resource.label())),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::ToggleDelegation(category) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    gameplay.sim.delegation.toggle(category);
                }
                None
            }
        }
    }

    fn advance_year(&mut self) {
        let GameState::Gameplay(gameplay) = &mut self.state else {
            return;
        };
        let sim = &mut gameplay.sim;
        if sim.has_pending_decision() || sim.dynasty.extinct {
            return;
        }

        let report = tick::advance_year(sim, &self.data);

        if report.decision_required {
            self.notifications.warning("The council must decide.");
        }
        if report.dynasty_extinct {
            self.notifications.danger("The dynasty has ended.");
        }
        if let Some((score, level)) = report.contract_completed {
            let entry = ChronicleEntry {
                completed_year: sim.year,
                contract_name: sim
                    .contract
                    .as_ref()
                    .map(|c| c.name.clone())
                    .unwrap_or_default(),
                objective: sim
                    .contract
                    .as_ref()
                    .map(|c| c.objective.label().to_owned())
                    .unwrap_or_default(),
                legacy_id: sim.legacy.legacy_id.clone(),
                leader_name: sim
                    .dynasty
                    .leader()
                    .map(|l| l.name.clone())
                    .unwrap_or_else(|| "an empty chair".to_owned()),
                generation: sim.dynasty.generation,
                score,
                outcome: level.label().to_owned(),
                duration_years: sim
                    .contract
                    .as_ref()
                    .map(|c| c.years_elapsed)
                    .unwrap_or_default(),
            };
            // Freeze the run timer for the Homecoming (PLAN M4.7).
            self.last_mission_real_secs = self.mission_started.map(|t| (get_time() - t) as f32);
            self.mission_started = None;
            sim.push_log(format!(
                "Contract concluded: {} — {} (score {score:.2}).",
                entry.contract_name, entry.outcome
            ));
            // Reward on any non-failure outcome.
            if level != contract::SuccessLevel::Failure {
                if let Some(template) = sim
                    .contract
                    .as_ref()
                    .and_then(|c| self.data.contracts.get(&c.template_id))
                {
                    sim.resources.apply(&template.reward);
                }
            }
            sim.contract = None;

            self.chronicle.record(entry);
            if let Err(err) = self.chronicle.save(
                &self.data.config.game_name,
                &self.data.config.chronicle_slot,
                &self.data.config.version,
            ) {
                self.notifications
                    .danger(format!("Chronicle write failed: {err}"));
            }
            self.notifications
                .success("Contract concluded — see the Chronicle.");
        }
    }

    fn purchase_component(&mut self, kind: ComponentKind, id: &str) {
        let GameState::Gameplay(gameplay) = &mut self.state else {
            return;
        };
        let Some(component) = self.data.ship_components.find(kind, id) else {
            return;
        };
        let sim = &mut gameplay.sim;

        // Loadout changes are a drydock job (PLAN M4.6): only in port.
        if sim.contract.is_some() {
            self.notifications
                .warning("Loadout changes wait for port — you're underway.");
            return;
        }

        let cost = crate::data::ResourceDelta {
            credits: -component.cost.credits,
            energy: -component.cost.energy,
            minerals: -component.cost.minerals,
            food: -component.cost.food,
            influence: -component.cost.influence,
        };
        if !sim.resources.can_afford(&cost) {
            self.notifications.warning("The treasury cannot cover it.");
            return;
        }
        sim.resources.apply(&cost);
        match kind {
            ComponentKind::Hull => sim.ship.hull = component.id.clone(),
            ComponentKind::Engine => sim.ship.engine = component.id.clone(),
            ComponentKind::Weapon => sim.ship.weapon = Some(component.id.clone()),
        }
        sim.push_log(format!("Refit complete: {} installed.", component.name));
        self.notifications
            .success(format!("{} installed.", component.name));
    }
}
