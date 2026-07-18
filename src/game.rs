//! Game struct: owns the state machine, applies UiActions, drives the tick.

use crate::chronicle::{ChronicleEntry, ChronicleStore};
use crate::data::ship_components::ComponentKind;
use crate::data::GameData;
use crate::save;
use crate::simulation::{contract, event_resolver, legacy, market, tick};
use crate::state::{GameState, GameplayState, MenuState, SimState, StateTransition};
use crate::ui::{self, UiAction};
use macroquad::prelude::*;
use macroquad_toolkit::assets::AssetManager;
use macroquad_toolkit::events::EventBus;
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
        }
    }

    /// Seed a deterministic state for the headless screenshot harness.
    pub fn begin_capture_scene(&mut self, scene: &str) {
        match scene {
            "menu" => self.state = GameState::Menu(MenuState::new(false)),
            "event" => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                sim.pending_event = Some(crate::state::sim::PendingEvent {
                    template_id: "cultural_schism".to_owned(),
                    rolled_year: 0,
                });
                self.state = GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "dilemma" => {
                let mut sim = SimState::new_campaign(&self.data, "preservers", 0xC0FFEE);
                sim.pending_dilemma = Some(crate::state::sim::PendingDilemma {
                    dilemma_id: "archive_purge".to_owned(),
                    rolled_year: 0,
                });
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
        clear_background(ui::term::BG);

        let virtual_ui = begin_virtual_ui_frame(ui::LOGICAL_WIDTH, ui::LOGICAL_HEIGHT);
        let actions = match &self.state {
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
            }),
        };
        end_virtual_ui_frame();

        for action in actions {
            self.events.push(action);
        }

        self.notifications
            .draw_with_config(&NotificationRenderConfig {
                anchor: NotificationAnchor::BottomRight,
                ..Default::default()
            });
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
