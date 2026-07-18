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
use macroquad_toolkit::assets::AssetManager;
use macroquad_toolkit::events::EventBus;
use macroquad_toolkit::fx::{CrtOverlay, CrtStyle};
use macroquad_toolkit::notifications::{
    NotificationAnchor, NotificationManager, NotificationRenderConfig,
};
use macroquad_toolkit::persistence::delete_slot;
use macroquad_toolkit::prelude::{begin_virtual_ui_frame, end_virtual_ui_frame};
use macroquad_toolkit::rng;

pub struct Game {
    data: GameData,
    state: GameState,
    chronicle: ChronicleStore,
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
    /// Whether the F1 display-settings overlay is open.
    settings_open: bool,
    /// Terminal typewriter reveal for blocking modals: which modal is showing
    /// and when it appeared, so its body text streams in. Purely cosmetic —
    /// never touches the deterministic sim.
    modal_key: Option<String>,
    modal_started: f64,
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
        let legacy_ids = GameData::sorted_ids(&data.legacies);
        let save_exists = save::save_exists(&data.config);
        let display = DisplaySettings::load(&data.config.game_name);
        let crt_style = display.crt_style();
        ui::term::set_phosphor(display.phosphor);

        let mut assets = AssetManager::new();
        let _ = assets.load_asset_pack("assets.zip").await;
        let _ = assets.load_texture_configs(&data.texture_manifest).await;

        Self {
            data,
            state: GameState::Menu(MenuState::new(save_exists)),
            chronicle,
            notifications: NotificationManager::new(),
            events: EventBus::new(),
            _assets: assets,
            legacy_ids,
            crt: CrtOverlay::new(),
            crt_style,
            display,
            settings_open: false,
            modal_key: None,
            modal_started: 0.0,
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
        self.boot.finish();
        self.display = DisplaySettings::default();
        self.crt_style = self.display.crt_style();
        ui::term::set_phosphor(self.display.phosphor);
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
                self.state = GameState::Menu(MenuState::new(true));
                self.settings_open = true;
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
                let sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = crate::state::Screen::ShipBuilder;
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

        let actions: Vec<UiAction> = self.events.drain().collect();
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
                    ui: &virtual_ui,
                }),
                GameState::Gameplay(gameplay) => ui::draw_gameplay(ui::GameplayCtx {
                    data: &self.data,
                    sim: &gameplay.sim,
                    screen: gameplay.screen,
                    chronicle: &self.chronicle,
                    ui: &virtual_ui,
                    modal_reveal,
                    log_reveal,
                }),
            }
        };

        // The display panel floats above everything and captures its input.
        let display_actions = if self.settings_open {
            ui::settings::draw(&self.display, virtual_ui.mouse_position())
        } else {
            Vec::new()
        };
        end_virtual_ui_frame();

        // While the panel is open, swallow the underlying screen's intents.
        if !self.settings_open {
            for action in actions {
                self.events.push(action);
            }
        }
        for action in display_actions {
            self.apply_display_action(action);
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

    fn transition(&mut self, transition: StateTransition) {
        match transition {
            StateTransition::NewCampaign { legacy_id, seed } => {
                let sim = SimState::new_campaign(&self.data, &legacy_id, seed);
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
                self.notifications
                    .success("The founding generation takes its oath.");
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
                if let (GameState::Gameplay(gameplay), Some(template)) =
                    (&mut self.state, self.data.contracts.get(&id))
                {
                    let sim = &mut gameplay.sim;
                    if sim.contract.is_none() {
                        sim.contract = Some(contract::start_contract(template, sim));
                        sim.push_log(format!("Charter accepted: {}", template.name));
                        self.notifications.success("Charter accepted.");
                    }
                }
                None
            }
            UiAction::PurchaseComponent(kind, id) => {
                self.purchase_component(kind, &id);
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
            };
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
